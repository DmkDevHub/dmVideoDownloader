use crate::{folders, settings::Store};
use dm_video_protocol::{
    manifest::{local_absolute, same_path, Manifest},
    pipe::RegistrationGuard,
    HOST_EXE, HOST_NAME, MANIFEST,
};
use std::{
    ffi::OsString,
    io::{self, Write},
    path::{Path, PathBuf},
};
use winreg::{
    enums::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE, KEY_WOW64_32KEY,
        KEY_WOW64_64KEY, REG_SZ,
    },
    types::FromRegValue,
    RegKey,
};

const CHROME_PARENT: &str = r"Software\Google\Chrome\NativeMessagingHosts";
const VIEWS: [u32; 2] = [KEY_WOW64_32KEY, KEY_WOW64_64KEY];
type Entries = [Option<PathBuf>; 2];

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
    fn read_entry(&self, hive: &RegKey, view: u32) -> Result<Option<PathBuf>, &'static str> {
        let key = match hive.open_subkey_with_flags(self.key(), KEY_READ | view) {
            Ok(key) => key,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("REGISTRATION_FAILED"),
        };
        let raw = match key.get_raw_value("") {
            Ok(value) => value,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("REGISTRATION_FAILED"),
        };
        if raw.vtype != REG_SZ || raw.bytes.len() > 65_536 {
            return Err("REGISTRATION_CONFLICT");
        }
        let path =
            PathBuf::from(OsString::from_reg_value(&raw).map_err(|_| "REGISTRATION_CONFLICT")?);
        if !local_absolute(&path) || path.to_string_lossy().contains('\0') {
            return Err("REGISTRATION_CONFLICT");
        }
        Ok(Some(path))
    }
    pub fn views(&self) -> Result<Entries, &'static str> {
        let user = RegKey::predef(HKEY_CURRENT_USER);
        Ok([
            self.read_entry(&user, VIEWS[0])?,
            self.read_entry(&user, VIEWS[1])?,
        ])
    }
    pub fn machine_registered(&self) -> bool {
        let machine = RegKey::predef(HKEY_LOCAL_MACHINE);
        VIEWS.iter().any(|&view| {
            matches!(
                self.read_entry(&machine, view),
                Ok(Some(_)) | Err("REGISTRATION_CONFLICT")
            )
        })
    }
    fn set_view(&self, path: &Path, view: u32) -> Result<(), &'static str> {
        let user = RegKey::predef(HKEY_CURRENT_USER);
        if self
            .read_entry(&user, view)?
            .is_some_and(|entry| same_path(&entry, path))
        {
            return Ok(());
        }
        let (key, _) = user
            .create_subkey_with_flags(self.key(), KEY_SET_VALUE | view)
            .map_err(|_| "REGISTRATION_FAILED")?;
        key.set_value("", &path.as_os_str())
            .map_err(|_| "REGISTRATION_FAILED")?;
        Ok(())
    }
    pub fn remove(&self, root: &Path) -> Result<(), &'static str> {
        let _guard = RegistrationGuard::acquire().map_err(|_| "REGISTRATION_BUSY")?;
        let expected = root.join(MANIFEST);
        let entries = self.views()?;
        if entries.iter().all(Option::is_none) {
            return Ok(());
        }
        if !entries
            .iter()
            .flatten()
            .any(|path| same_path(path, &expected))
        {
            return Err("REGISTRATION_CONFLICT");
        }
        folders::check_path(&expected)?;
        let own = Manifest::read(&expected).map_err(|_| "REGISTRATION_CONFLICT")?;
        if !same_path(&own.path, &root.join(HOST_EXE)) {
            return Err("REGISTRATION_CONFLICT");
        }
        let user = RegKey::predef(HKEY_CURRENT_USER);
        for view in VIEWS {
            if !self
                .read_entry(&user, view)?
                .is_some_and(|path| same_path(&path, &expected))
            {
                continue;
            }
            let key = user
                .open_subkey_with_flags(self.key(), KEY_READ | KEY_SET_VALUE | view)
                .map_err(|_| "REGISTRATION_FAILED")?;
            key.delete_value("").map_err(|_| "REGISTRATION_FAILED")?;
            let empty = key
                .query_info()
                .is_ok_and(|info| info.values == 0 && info.sub_keys == 0);
            drop(key);
            if empty {
                let _ = user.delete_subkey_with_flags(self.key(), view);
            }
        }
        if self
            .views()?
            .iter()
            .flatten()
            .any(|entry| same_path(entry, &expected))
        {
            return Err("REGISTRATION_UNCONFIRMED");
        }
        Ok(())
    }
    pub fn status(&self, root: &Path, id: &str) -> Result<(), &'static str> {
        let entries = self.views()?;
        if let [Some(first), Some(second)] = &entries {
            if !same_path(first, second) {
                return Err("REGISTRY_VIEW_MISMATCH");
            }
        }
        let path = entries.iter().flatten().next().ok_or_else(|| {
            if self.machine_registered() {
                "MACHINE_REGISTRATION_PRESENT"
            } else {
                "HOST_NOT_REGISTERED"
            }
        })?;
        if !same_path(path, &root.join(MANIFEST)) {
            return Err("HOST_PATH_MISMATCH");
        }
        if !root.join(HOST_EXE).is_file() {
            return Err("HOST_MISSING");
        }
        folders::check_path(path)?;
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
        entries: &Entries,
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
        for registered in entries
            .iter()
            .flatten()
            .filter(|value| !same_path(value, &path))
        {
            if !repair {
                return Err("REGISTRATION_CONFLICT");
            }
            folders::check_path(registered)?;
            let owned_path = |entry: &Manifest| {
                entry
                    .path
                    .parent()
                    .is_some_and(|parent| same_path(registered, &parent.join(MANIFEST)))
            };
            let owned = if registered.exists() {
                Manifest::read(registered).is_ok_and(|entry| owned_path(&entry))
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
        let _guard = RegistrationGuard::acquire().map_err(|_| "REGISTRATION_BUSY")?;
        let root = store.root().to_path_buf();
        let mut settings = store.current_settings()?;
        if settings.browser_extension_id == id && self.status(&root, id).is_ok() {
            return Ok(());
        }
        let entries = self.views()?;
        let (temp, path) = self.prepare(&root, id, repair, &entries)?;
        if settings.browser_extension_id != id {
            settings.browser_extension_id = id.to_owned();
            store.save(settings)?;
        }
        // Recheck after the settings save so a concurrent external edit is visible.
        if self.views()? != entries {
            return Err("REGISTRATION_CONFLICT");
        }
        temp.persist(&path).map_err(|_| "REGISTRATION_FAILED")?;
        self.set_view(&path, VIEWS[0])?;
        if entries[1].is_some() {
            self.set_view(&path, VIEWS[1])?;
        }
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
        assert_eq!(registry.views().unwrap().into_iter().flatten().next(), None);
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
    assert_eq!(
        registry.views().unwrap().into_iter().flatten().next(),
        Some(new.join(MANIFEST))
    );
    assert_eq!(registry.status(&new, &old_id), Err("EXTENSION_ID_MISMATCH"));
    assert_eq!(registry.status(&new, &new_id), Ok(()));
    registry.remove(&new).unwrap();
}

#[cfg(test)]
#[test]
fn redirected_views_keep_foreign_copy_and_repair_stale_registration() {
    let fixture = tempfile::tempdir().unwrap();
    // CLSID is redirected by WOW64. This private test name is not a COM class ID.
    let parent = format!(
        r"Software\Classes\CLSID\DmVideoDownloaderTest-{}",
        fixture.path().file_name().unwrap().to_string_lossy()
    );
    let registry = Registry {
        parent: parent.clone(),
    };
    struct Cleanup {
        key: String,
        parent: String,
    }
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let user = RegKey::predef(HKEY_CURRENT_USER);
            for view in VIEWS {
                let _ = user.delete_subkey_with_flags(&self.key, view);
                let _ = user.delete_subkey_with_flags(&self.parent, view);
            }
        }
    }
    let _cleanup = Cleanup {
        key: registry.key(),
        parent,
    };
    let id = "c".repeat(32);
    let make_copy = |name: &str| {
        let root = fixture.path().join(name);
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join(HOST_EXE), b"fixture").unwrap();
        let manifest = Manifest::new(&root, &id).unwrap();
        std::fs::write(root.join(MANIFEST), serde_json::to_vec(&manifest).unwrap()).unwrap();
        root
    };
    let first = make_copy("first");
    let second = make_copy("second");
    registry.set_view(&first.join(MANIFEST), VIEWS[0]).unwrap();
    registry.set_view(&second.join(MANIFEST), VIEWS[1]).unwrap();
    assert_eq!(
        registry.views().unwrap(),
        [Some(first.join(MANIFEST)), Some(second.join(MANIFEST))]
    );
    assert_eq!(registry.status(&first, &id), Err("REGISTRY_VIEW_MISMATCH"));
    registry.remove(&first).unwrap();
    assert_eq!(
        registry.views().unwrap(),
        [None, Some(second.join(MANIFEST))]
    );
    registry.set_view(&first.join(MANIFEST), VIEWS[0]).unwrap();
    let mut store = Store::new(
        first.clone(),
        fixture
            .path()
            .join("Downloads")
            .to_string_lossy()
            .into_owned(),
    );
    store.load().unwrap();
    assert_eq!(
        registry.register(&mut store, &id, false),
        Err("REGISTRATION_CONFLICT")
    );
    registry.register(&mut store, &id, true).unwrap();
    let expected = [Some(first.join(MANIFEST)), Some(first.join(MANIFEST))];
    assert_eq!(registry.views().unwrap(), expected);
    let before = std::fs::read(first.join(MANIFEST)).unwrap();
    registry.register(&mut store, &id, true).unwrap();
    assert_eq!(registry.views().unwrap(), expected);
    assert_eq!(std::fs::read(first.join(MANIFEST)).unwrap(), before);
    assert_eq!(registry.remove(&second), Err("REGISTRATION_CONFLICT"));
    let user = RegKey::predef(HKEY_CURRENT_USER);
    let key = user
        .open_subkey_with_flags(registry.key(), KEY_SET_VALUE | VIEWS[0])
        .unwrap();
    key.set_value("unrelated", &"keep").unwrap();
    drop(key);
    registry.remove(&first).unwrap();
    assert_eq!(registry.views().unwrap(), [None, None]);
    let key = user
        .open_subkey_with_flags(registry.key(), KEY_READ | VIEWS[0])
        .unwrap();
    assert_eq!(key.get_value::<String, _>("unrelated").unwrap(), "keep");
    assert!(second.join(MANIFEST).is_file());
}
