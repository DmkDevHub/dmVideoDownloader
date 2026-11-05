use crate::folders;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub schema_version: u32,
    pub download_directory: String,
    pub quality: Quality,
    pub format: Format,
    pub filename: Filename,
    pub concurrent_downloads: u8,
    pub retry_count: u8,
    pub browser_auto_detect: bool,
    #[serde(default)]
    pub browser_extension_id: String,
    pub completion_notification: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Quality {
    #[serde(rename = "best")]
    Best,
    #[serde(rename = "2160")]
    Q2160,
    #[serde(rename = "1080")]
    Q1080,
    #[serde(rename = "720")]
    Q720,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Auto,
    Mp4,
    Mkv,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Filename {
    #[serde(rename = "title")]
    Title,
    #[serde(rename = "title-id")]
    TitleId,
    #[serde(rename = "date-title")]
    DateTitle,
}

impl Settings {
    pub fn defaults(directory: String) -> Self {
        Self {
            schema_version: 1,
            download_directory: directory,
            quality: Quality::Best,
            format: Format::Auto,
            filename: Filename::Title,
            concurrent_downloads: 2,
            retry_count: 3,
            browser_auto_detect: true,
            browser_extension_id: String::new(),
            completion_notification: true,
        }
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != 1 {
            return Err("UNSUPPORTED_SETTINGS_VERSION");
        }
        if !(1..=4).contains(&self.concurrent_downloads)
            || ![0, 1, 3, 5].contains(&self.retry_count)
        {
            return Err("INVALID_SETTINGS");
        }
        let id = &self.browser_extension_id;
        if !id.is_empty() && (id.len() != 32 || !id.bytes().all(|b| (b'a'..=b'p').contains(&b))) {
            return Err("INVALID_EXTENSION_ID");
        }
        folders::check_path(Path::new(&self.download_directory))
    }
}

pub fn parse(bytes: &[u8]) -> Result<Settings, &'static str> {
    let raw: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| "INVALID_SETTINGS")?;
    if raw
        .get("schemaVersion")
        .and_then(|v| v.as_u64())
        .is_some_and(|version| version > 1)
    {
        return Err("UNSUPPORTED_SETTINGS_VERSION");
    }
    let settings: Settings = serde_json::from_slice(bytes).map_err(|_| "INVALID_SETTINGS")?;
    settings.validate()?;
    Ok(settings)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub settings: Settings,
    pub defaults: Settings,
    pub notice: Option<String>,
    pub can_save: bool,
    pub can_recover: bool,
}

pub struct Store {
    root: PathBuf,
    pub defaults: Settings,
    baseline: Option<Vec<u8>>,
}

impl Store {
    pub fn new(root: PathBuf, directory: String) -> Self {
        Self {
            root,
            defaults: Settings::defaults(directory),
            baseline: None,
        }
    }
    pub fn path(&self) -> PathBuf {
        self.root.join("data").join("settings.json")
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn saved_directory(&self) -> Result<PathBuf, &'static str> {
        let _guard = self.lock()?;
        Ok(PathBuf::from(parse(&self.read()?)?.download_directory))
    }
    fn lock(&self) -> Result<File, &'static str> {
        let data = self.root.join("data");
        folders::check_path(&data)?;
        fs::create_dir_all(&data).map_err(|_| "SETTINGS_NOT_WRITABLE")?;
        folders::check_path(&data)?;
        folders::check_path(&self.path())?;
        let lock_path = data.join("settings.lock");
        folders::check_path(&lock_path)?;
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .open(lock_path)
            .map_err(|error| {
                if matches!(error.raw_os_error(), Some(32 | 33)) {
                    "SETTINGS_BUSY"
                } else {
                    "SETTINGS_NOT_WRITABLE"
                }
            })
    }
    fn read(&self) -> Result<Vec<u8>, &'static str> {
        let mut bytes = Vec::new();
        File::open(self.path())
            .map_err(|_| "SETTINGS_READ_FAILED")?
            .take(65537)
            .read_to_end(&mut bytes)
            .map_err(|_| "SETTINGS_READ_FAILED")?;
        if bytes.len() > 65536 {
            return Err("SETTINGS_TOO_LARGE");
        }
        Ok(bytes)
    }
    fn write(&self, settings: &Settings, initial: bool) -> Result<Vec<u8>, &'static str> {
        let bytes = serde_json::to_vec_pretty(settings).map_err(|_| "INVALID_SETTINGS")?;
        let mut temp = tempfile::Builder::new()
            .prefix(".settings-")
            .tempfile_in(self.root.join("data"))
            .map_err(|_| "SETTINGS_NOT_WRITABLE")?;
        temp.write_all(&bytes)
            .and_then(|_| temp.as_file().sync_all())
            .map_err(|_| "SETTINGS_NOT_WRITABLE")?;
        if initial {
            temp.persist_noclobber(self.path())
                .map_err(|_| "SETTINGS_NOT_WRITABLE")?;
        } else {
            temp.persist(self.path())
                .map_err(|_| "SETTINGS_NOT_WRITABLE")?;
        }
        if self.read()? != bytes {
            return Err("SETTINGS_WRITE_UNCONFIRMED");
        }
        Ok(bytes)
    }
    pub fn load(&mut self) -> Result<Snapshot, &'static str> {
        let _guard = self.lock()?;
        let bytes = if self
            .path()
            .try_exists()
            .map_err(|_| "SETTINGS_READ_FAILED")?
        {
            self.read()?
        } else {
            self.defaults.validate()?;
            folders::validate_directory(&self.defaults.download_directory)?;
            self.write(&self.defaults, true)?
        };
        let parsed = parse(&bytes);
        self.baseline = Some(bytes);
        match parsed {
            Ok(settings) => Ok(Snapshot {
                settings,
                defaults: self.defaults.clone(),
                notice: None,
                can_save: true,
                can_recover: false,
            }),
            Err(code) => Ok(Snapshot {
                settings: self.defaults.clone(),
                defaults: self.defaults.clone(),
                notice: Some(code.to_owned()),
                can_save: false,
                can_recover: code != "UNSUPPORTED_SETTINGS_VERSION",
            }),
        }
    }
    pub fn save(&mut self, settings: Settings) -> Result<Settings, &'static str> {
        settings.validate()?;
        let _guard = self.lock()?;
        let baseline = self.baseline.as_ref().ok_or("SETTINGS_NOT_LOADED")?;
        parse(baseline)?;
        if self.read()? != *baseline {
            return Err("SETTINGS_CHANGED");
        }
        folders::validate_directory(&settings.download_directory)?;
        self.baseline = Some(self.write(&settings, false)?);
        Ok(settings)
    }

    pub fn recover(&mut self) -> Result<Snapshot, &'static str> {
        let _guard = self.lock()?;
        let before = self.baseline.as_ref().ok_or("SETTINGS_NOT_LOADED")?;
        if self.read()? != *before {
            return Err("SETTINGS_CHANGED");
        }
        match parse(before) {
            Ok(_) => return Err("RECOVERY_NOT_REQUIRED"),
            Err("UNSUPPORTED_SETTINGS_VERSION") => return Err("UNSUPPORTED_SETTINGS_VERSION"),
            Err(_) => (),
        }
        self.defaults.validate()?;
        folders::validate_directory(&self.defaults.download_directory)?;
        let mut backup = tempfile::Builder::new()
            .prefix("settings.backup-")
            .suffix(".json")
            .tempfile_in(self.root.join("data"))
            .map_err(|_| "BACKUP_FAILED")?;
        backup
            .write_all(before)
            .and_then(|_| backup.as_file().sync_all())
            .map_err(|_| "BACKUP_FAILED")?;
        backup.keep().map_err(|_| "BACKUP_FAILED")?;
        self.baseline = Some(self.write(&self.defaults, false)?);
        Ok(Snapshot {
            settings: self.defaults.clone(),
            defaults: self.defaults.clone(),
            notice: Some("SETTINGS_RECOVERED".into()),
            can_save: true,
            can_recover: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store(root: &Path) -> Store {
        Store::new(
            root.to_path_buf(),
            root.join("Downloads")
                .join("Videos")
                .to_string_lossy()
                .into_owned(),
        )
    }
    #[test]
    fn creates_and_atomically_replaces_then_loads_in_another_instance() {
        let root = tempfile::tempdir().unwrap();
        let mut first = store(root.path());
        let mut settings = first.load().unwrap().settings;
        settings.quality = Quality::Q1080;
        settings.download_directory = root
            .path()
            .join("한글 Videos")
            .to_string_lossy()
            .into_owned();
        first.save(settings.clone()).unwrap();
        assert_eq!(store(root.path()).load().unwrap().settings, settings);
        assert_eq!(fs::read_dir(root.path().join("data")).unwrap().count(), 2);
    }
    #[test]
    fn rejects_stale_writer_and_locked_target_without_losing_previous_data() {
        let root = tempfile::tempdir().unwrap();
        let mut first = store(root.path());
        let mut stale = store(root.path());
        let mut settings = first.load().unwrap().settings;
        stale.load().unwrap();
        settings.retry_count = 5;
        first.save(settings.clone()).unwrap();
        assert_eq!(stale.save(settings.clone()), Err("SETTINGS_CHANGED"));
        let before = fs::read(first.path()).unwrap();
        let held = OpenOptions::new()
            .read(true)
            .share_mode(3)
            .open(first.path())
            .unwrap();
        settings.retry_count = 0;
        assert_eq!(first.save(settings), Err("SETTINGS_NOT_WRITABLE"));
        drop(held);
        assert_eq!(fs::read(first.path()).unwrap(), before);
    }
    #[test]
    fn schema_rejects_invalid_types_ranges_unknown_fields_and_duplicates() {
        let settings = Settings::defaults("D:\\Videos".into());
        let original = serde_json::to_value(&settings).unwrap();
        for (key, value) in [
            ("schemaVersion", serde_json::json!(2)),
            ("quality", serde_json::json!("480")),
            ("concurrentDownloads", serde_json::json!(0)),
            ("retryCount", serde_json::json!(2)),
            ("browserAutoDetect", serde_json::json!("true")),
            ("browserExtensionId", serde_json::json!("*")),
            ("cookies", serde_json::json!("not-allowed")),
        ] {
            let mut candidate = original.clone();
            candidate[key] = value;
            assert!(
                parse(&serde_json::to_vec(&candidate).unwrap()).is_err(),
                "{key}"
            );
        }
        let mut missing = original.clone();
        missing.as_object_mut().unwrap().remove("quality");
        assert!(parse(&serde_json::to_vec(&missing).unwrap()).is_err());
        let mut legacy = original;
        legacy.as_object_mut().unwrap().remove("browserExtensionId");
        assert_eq!(
            parse(&serde_json::to_vec(&legacy).unwrap())
                .unwrap()
                .browser_extension_id,
            ""
        );
        let duplicate = String::from_utf8(serde_json::to_vec(&settings).unwrap())
            .unwrap()
            .replacen('{', "{\"schemaVersion\":1,", 1);
        assert!(parse(duplicate.as_bytes()).is_err());
    }

    #[test]
    fn corruption_is_read_only_until_explicit_backup_and_recovery() {
        let root = tempfile::tempdir().unwrap();
        let mut service = store(root.path());
        service.load().unwrap();
        let damaged = b"{broken-json";
        fs::write(service.path(), damaged).unwrap();
        let snapshot = service.load().unwrap();
        assert!(!snapshot.can_save && snapshot.can_recover);
        assert_eq!(fs::read(service.path()).unwrap(), damaged);
        assert!(service.save(snapshot.settings).is_err());
        let recovered = service.recover().unwrap();
        assert!(recovered.can_save && !recovered.can_recover);
        let backup = fs::read_dir(root.path().join("data"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("settings.backup-")
            })
            .unwrap();
        assert_eq!(fs::read(backup).unwrap(), damaged);
        assert_eq!(
            store(root.path()).load().unwrap().settings,
            service.defaults
        );
    }

    #[test]
    fn future_schema_and_concurrent_lock_are_preserved() {
        let root = tempfile::tempdir().unwrap();
        let mut service = store(root.path());
        service.load().unwrap();
        let future = b"{\"schemaVersion\":99,\"futureField\":true}";
        fs::write(service.path(), future).unwrap();
        let loaded = service.load().unwrap();
        assert!(!loaded.can_save && !loaded.can_recover);
        assert!(matches!(
            service.recover(),
            Err("UNSUPPORTED_SETTINGS_VERSION")
        ));
        assert_eq!(fs::read(service.path()).unwrap(), future);
        let held = service.lock().unwrap();
        assert!(matches!(store(root.path()).load(), Err("SETTINGS_BUSY")));
        drop(held);
    }
}
