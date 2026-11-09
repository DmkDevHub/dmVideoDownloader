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
            dm_video_protocol::Code::HostNotRegistered => "MANIFEST_MISSING",
            _ => "REGISTRATION_FAILED",
        })
    }
    fn prepare(
        &self,
        root: &Path,
        id: &str,
        repair: bool,
    ) -> Result<(tempfile::NamedTempFile, PathBuf), &'static str> {
        let manifest = Manifest::new(root, id).map_err(|_| "INVALID_EXTENSION_ID")?;
        let path = root.join(MANIFEST);
        for checked in [root, &manifest.path, &path] {
            folders::check_path(checked)?;
        }
        if !manifest.path.is_file() {
            return Err("HOST_MISSING");
        }
        let previous = if path.exists() {
            Some(Manifest::read(&path).map_err(|_| "REGISTRATION_CONFLICT")?)
        } else {
            None
        };
        if let Some(registered) = self.read()?.filter(|value| !same_path(value, &path)) {
            if !repair {
                return Err("REGISTRATION_CONFLICT");
            }
            folders::check_path(&registered)?;
            let owned_path = |entry: &Manifest| {
                entry
                    .path
                    .parent()
                    .is_some_and(|parent| same_path(&registered, &parent.join(MANIFEST)))
            };
            let owned = if registered.exists() {
                Manifest::read(&registered).is_ok_and(|entry| owned_path(&entry))
            } else {
                previous.as_ref().is_some_and(owned_path)
            };
            if !owned {
                return Err("REGISTRATION_CONFLICT");
            }
        }
        if !repair
            && previous
                .as_ref()
                .is_some_and(|prior| !same_path(&prior.path, &manifest.path))
        {
            return Err("HOST_PATH_MISMATCH");
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
    pub fn register(&self, store: &mut Store, id: &str, repair: bool) -> Result<(), &'static str> {
        let root = store.root().to_path_buf();
        let (temp, path) = self.prepare(&root, id, repair)?;
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
            registry.register(&mut store, "*", false),
            Err("INVALID_EXTENSION_ID")
        );
        assert_eq!(registry.read().unwrap(), None);
        registry.register(&mut store, &id, false).unwrap();
        assert_eq!(registry.status(root, &id), Ok(()));
        assert_eq!(Manifest::read(&root.join(MANIFEST)).unwrap().id(), id);
        std::fs::rename(root.join(MANIFEST), root.join("test-manifest-backup.json")).unwrap();
        assert_eq!(registry.status(root, &id), Err("MANIFEST_MISSING"));
        registry.register(&mut store, &id, true).unwrap();
        assert_eq!(registry.status(root, &id), Ok(()));
        std::fs::rename(root.join(HOST_EXE), root.join("test-host-backup.exe")).unwrap();
        assert_eq!(registry.status(root, &id), Err("HOST_MISSING"));
        std::fs::rename(root.join("test-host-backup.exe"), root.join(HOST_EXE)).unwrap();
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

#[cfg(test)]
#[test]
fn moved_install_repairs_absolute_path_and_replaces_id() {
    let sandbox = tempfile::tempdir().unwrap();
    let old = sandbox.path().join("이전 폴더");
    let new = sandbox.path().join("새 위치");
    std::fs::create_dir(&old).unwrap();
    std::fs::write(old.join(HOST_EXE), b"fixture").unwrap();
    let downloads = sandbox
        .path()
        .join("Downloads")
        .to_string_lossy()
        .into_owned();
    let mut store = Store::new(old.clone(), downloads.clone());
    store.load().unwrap();
    let name = format!(
        r"Software\DmVideoDownloaderMoveTest-{}",
        sandbox.path().file_name().unwrap().to_string_lossy()
    );
    let registry = Registry {
        parent: name.clone(),
    };
    struct Cleanup(String);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.0);
        }
    }
    let _cleanup = Cleanup(name);
    let old_id = "a".repeat(32);
    let new_id = "b".repeat(32);
    registry.register(&mut store, &old_id, false).unwrap();
    std::fs::rename(&old, &new).unwrap();
    assert!(!old.exists());
    let mut store = Store::new(new.clone(), downloads);
    store.load().unwrap();
    assert_eq!(registry.status(&new, &old_id), Err("HOST_PATH_MISMATCH"));
    assert_eq!(
        registry.register(&mut store, &new_id, false),
        Err("REGISTRATION_CONFLICT")
    );
    registry.register(&mut store, &new_id, true).unwrap();
    assert_eq!(registry.read().unwrap(), Some(new.join(MANIFEST)));
    assert_eq!(registry.status(&new, &old_id), Err("EXTENSION_ID_MISMATCH"));
    assert_eq!(registry.status(&new, &new_id), Ok(()));
    registry.remove(&new).unwrap();
}
