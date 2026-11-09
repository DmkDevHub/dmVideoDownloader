use crate::{folders, settings::Store};
use dm_video_protocol::{
    manifest::{local_absolute, same_path, Manifest},
    HOST_EXE, HOST_NAME, MANIFEST,
};
use std::{
    ffi::OsString,
    io::{self, Write},
    path::{Path, PathBuf},
};
use winreg::{
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, KEY_WOW64_32KEY},
    RegKey,
};

const CHROME_PARENT: &str = r"Software\Google\Chrome\NativeMessagingHosts";
pub struct Registry {
    parent: String,
}
impl Registry {
    pub fn chrome() -> Self {
        Self {
            parent: CHROME_PARENT.into(),
        }
    }
    fn key(&self) -> String {
        format!(r"{}\{HOST_NAME}", self.parent)
    }
    pub fn read(&self) -> Result<Option<PathBuf>, &'static str> {
        let key = match RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(self.key(), KEY_READ | KEY_WOW64_32KEY)
        {
            Ok(key) => key,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("REGISTRATION_FAILED"),
        };
        match key.get_value::<OsString, _>("") {
            Ok(path) if local_absolute(Path::new(&path)) => Ok(Some(path.into())),
            Ok(_) => Err("REGISTRATION_CONFLICT"),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err("REGISTRATION_CONFLICT"),
        }
    }
    fn write(&self, path: &Path) -> Result<(), &'static str> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey_with_flags(self.key(), KEY_SET_VALUE | KEY_WOW64_32KEY)
            .map_err(|_| "REGISTRATION_FAILED")?;
        key.set_value("", &path.as_os_str())
            .map_err(|_| "REGISTRATION_FAILED")?;
        if self.read()?.is_some_and(|value| same_path(&value, path)) {
            Ok(())
        } else {
            Err("REGISTRATION_UNCONFIRMED")
        }
    }
    pub fn remove(&self, root: &Path) -> Result<(), &'static str> {
        let Some(path) = self.read()? else {
            return Ok(());
        };
        if !same_path(&path, &root.join(MANIFEST)) {
            return Err("REGISTRATION_CONFLICT");
        }
        folders::check_path(&path)?;
        let own = Manifest::read(&path).map_err(|_| "REGISTRATION_CONFLICT")?;
        if !same_path(&own.path, &root.join(HOST_EXE)) {
            return Err("REGISTRATION_CONFLICT");
        }
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(self.key(), KEY_SET_VALUE | KEY_WOW64_32KEY)
            .map_err(|_| "REGISTRATION_FAILED")?;
        key.delete_value("").map_err(|_| "REGISTRATION_FAILED")?;
        if self.read()?.is_some() {
            return Err("REGISTRATION_UNCONFIRMED");
        }
        Ok(())
    }
    pub fn status(&self, root: &Path, id: &str) -> Result<(), &'static str> {
        let path = self.read()?.ok_or("HOST_NOT_REGISTERED")?;
        if !same_path(&path, &root.join(MANIFEST)) {
            return Err("HOST_PATH_MISMATCH");
        }
        if !root.join(HOST_EXE).is_file() {
            return Err("HOST_MISSING");
        }
        folders::check_path(&path)?;
        Manifest::verify(root, id).map_err(|error| match error {
            dm_video_protocol::Code::ExtensionIdMismatch => "EXTENSION_ID_MISMATCH",
            dm_video_protocol::Code::HostPathMismatch => "HOST_PATH_MISMATCH",
            _ => "REGISTRATION_FAILED",
        })
    }
    fn prepare(
        &self,
        root: &Path,
        id: &str,
    ) -> Result<(tempfile::NamedTempFile, PathBuf), &'static str> {
        let manifest = Manifest::new(root, id).map_err(|_| "INVALID_EXTENSION_ID")?;
        let path = root.join(MANIFEST);
        for checked in [root, &manifest.path, &path] {
            folders::check_path(checked)?;
        }
        if !manifest.path.is_file() {
            return Err("HOST_MISSING");
        }
        if self.read()?.is_some_and(|value| !same_path(&value, &path)) {
            return Err("REGISTRATION_CONFLICT");
        }
        if path.exists() {
            let prior = Manifest::read(&path).map_err(|_| "REGISTRATION_CONFLICT")?;
            if !same_path(&prior.path, &manifest.path) {
                return Err("HOST_PATH_MISMATCH");
            }
        }
        let mut temp = tempfile::Builder::new()
            .prefix(".native-host-")
            .tempfile_in(root)
            .map_err(|_| "REGISTRATION_FAILED")?;
        serde_json::to_writer_pretty(&mut temp, &manifest).map_err(|_| "REGISTRATION_FAILED")?;
        temp.flush()
            .and_then(|_| temp.as_file().sync_all())
            .map_err(|_| "REGISTRATION_FAILED")?;
        Manifest::read(temp.path()).map_err(|_| "REGISTRATION_FAILED")?;
        Ok((temp, path))
    }
    pub fn register(&self, store: &mut Store, id: &str) -> Result<(), &'static str> {
        let root = store.root().to_path_buf();
        let (temp, path) = self.prepare(&root, id)?;
        let mut settings = store.current_settings()?;
        if settings.browser_extension_id != id {
            settings.browser_extension_id = id.to_owned();
            store.save(settings)?;
        }
        temp.persist(&path).map_err(|_| "REGISTRATION_FAILED")?;
        self.write(&path)?;
        self.status(&root, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isolated_hkcu_registration_and_owned_removal() {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path();
        let key = format!(
            r"Software\DmVideoDownloaderTest-{}",
            root.file_name().unwrap().to_string_lossy()
        );
        let registry = Registry {
            parent: key.clone(),
        };
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.0);
            }
        }
        let _cleanup = Cleanup(key);
        std::fs::write(root.join(HOST_EXE), b"fixture").unwrap();
        let mut store = Store::new(
            root.into(),
            root.join("Downloads").to_string_lossy().into_owned(),
        );
        store.load().unwrap();
        let id = "a".repeat(32);
        assert_eq!(
            registry.register(&mut store, "*"),
            Err("INVALID_EXTENSION_ID")
        );
        assert_eq!(registry.read().unwrap(), None);
        registry.register(&mut store, &id).unwrap();
        assert_eq!(registry.status(root, &id), Ok(()));
        assert_eq!(Manifest::read(&root.join(MANIFEST)).unwrap().id(), id);
        assert_eq!(
            registry.status(root, &"b".repeat(32)),
            Err("EXTENSION_ID_MISMATCH")
        );
        let other = tempfile::tempdir().unwrap();
        assert_eq!(registry.remove(other.path()), Err("REGISTRATION_CONFLICT"));
        registry.remove(root).unwrap();
        assert_eq!(registry.status(root, &id), Err("HOST_NOT_REGISTERED"));
        assert_eq!(store.current_settings().unwrap().browser_extension_id, id);
    }
}
