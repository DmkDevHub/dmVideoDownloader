use crate::{extension_id, extension_origin, Code, HOST_EXE, HOST_NAME, MANIFEST};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::Read,
    path::{Component, Path, PathBuf, Prefix},
};

const DESCRIPTION: &str = "Browser Video Downloader bridge";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    name: String,
    description: String,
    pub path: PathBuf,
    #[serde(rename = "type")]
    kind: String,
    allowed_origins: [String; 1],
}
pub fn local_absolute(path: &Path) -> bool {
    path.is_absolute()
        && matches!(path.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_)))
        && !path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
}
pub fn same_path(left: &Path, right: &Path) -> bool {
    if left.to_string_lossy().to_lowercase() == right.to_string_lossy().to_lowercase() {
        return true;
    }
    matches!((crate::pipe::canonical(left), crate::pipe::canonical(right)), (Ok(a), Ok(b)) if a == b)
}
impl Manifest {
    pub fn new(root: &Path, id: &str) -> Result<Self, Code> {
        if !extension_id(id) {
            return Err(Code::ExtensionIdMismatch);
        }
        if !local_absolute(root) {
            return Err(Code::HostPathMismatch);
        }
        Ok(Self {
            name: HOST_NAME.into(),
            description: DESCRIPTION.into(),
            path: root.join(HOST_EXE),
            kind: "stdio".into(),
            allowed_origins: [format!("chrome-extension://{id}/")],
        })
    }
    pub fn read(path: &Path) -> Result<Self, Code> {
        if !local_absolute(path) {
            return Err(Code::HostPathMismatch);
        }
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    Code::HostNotRegistered
                } else {
                    Code::RegistrationFailed
                }
            })?
            .take(8193)
            .read_to_end(&mut bytes)
            .map_err(|_| Code::RegistrationFailed)?;
        if bytes.len() > 8192 {
            return Err(Code::RegistrationFailed);
        }
        let value: Self = serde_json::from_slice(&bytes).map_err(|_| Code::RegistrationFailed)?;
        if value.name != HOST_NAME
            || value.description != DESCRIPTION
            || value.kind != "stdio"
            || !local_absolute(&value.path)
            || value
                .path
                .file_name()
                .is_none_or(|name| !name.eq_ignore_ascii_case(HOST_EXE))
            || extension_origin(&value.allowed_origins[0]).is_none()
        {
            return Err(Code::RegistrationFailed);
        }
        Ok(value)
    }
    pub fn id(&self) -> &str {
        extension_origin(&self.allowed_origins[0]).expect("validated origin")
    }
    pub fn verify(root: &Path, id: &str) -> Result<(), Code> {
        let value = Self::read(&root.join(MANIFEST))?;
        if !same_path(&value.path, &root.join(HOST_EXE)) {
            return Err(Code::HostPathMismatch);
        }
        if value.id() != id {
            return Err(Code::ExtensionIdMismatch);
        }
        Ok(())
    }
}
