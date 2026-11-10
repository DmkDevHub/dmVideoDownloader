use super::{FileIdentity, PartialFile};
use crate::folders::check_path;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use url::Url;

const MAGIC: &[u8; 8] = b"DMRESM01";
const FOOTER_BYTES: u64 = 112;

pub(super) struct Pending {
    path: PathBuf,
    key: [u8; 32],
}

fn identity_hash(identity: &FileIdentity) -> [u8; 32] {
    Sha256::new()
        .chain_update(identity.source.as_str())
        .chain_update([0])
        .chain_update(identity.destination.as_str())
        .chain_update([0])
        .chain_update(identity.etag.as_bytes())
        .finalize()
        .into()
}

impl Pending {
    fn new(directory: &Path, url: &Url) -> Self {
        let digest = Sha256::digest(url.as_str());
        Self {
            path: directory.join(format!(".dm-resume-{}.part", &format!("{digest:x}")[..32])),
            key: digest.into(),
        }
    }

    fn read_footer(&self, file: &mut File) -> io::Result<(u64, [u8; FOOTER_BYTES as usize])> {
        let offset = file
            .metadata()?
            .len()
            .checked_sub(FOOTER_BYTES)
            .filter(|offset| *offset > 0)
            .ok_or(io::ErrorKind::InvalidData)?;
        let mut footer = [0; FOOTER_BYTES as usize];
        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(&mut footer)?;
        if &footer[..8] != MAGIC || footer[8..40] != self.key {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok((offset, footer))
    }

    pub(super) fn find(directory: &Path, url: &Url) -> Option<Self> {
        let pending = Self::new(directory, url);
        check_path(&pending.path).ok()?;
        if !pending.path.is_file() {
            return None;
        }
        pending
            .read_footer(&mut File::open(&pending.path).ok()?)
            .ok()?;
        Some(pending)
    }

    pub(super) fn claim(self, identity: Option<FileIdentity>) -> Option<PartialFile> {
        check_path(&self.path).ok()?;
        let temporary = tempfile::Builder::new()
            .prefix(".dm-part-")
            .tempfile_in(self.path.parent()?)
            .ok()?
            .into_temp_path();
        // The target is our own empty temporary file. Renaming claims the checkpoint for
        // one job and leaves the original name available for a later interruption.
        fs::rename(&self.path, &temporary).ok()?;
        let identity = identity?;
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&temporary)
            .ok()?;
        let (downloaded, footer) = self.read_footer(&mut file).ok()?;
        if footer[40..72] != identity_hash(&identity)
            || footer[72..80] != identity.length.to_le_bytes()
            || downloaded >= identity.length
            || downloaded > 2 * 1024 * 1024 * 1024
        {
            return None;
        }
        file.rewind().ok()?;
        let mut digest = Sha256::new();
        let mut left = downloaded;
        let mut buffer = [0; 64 * 1024];
        while left != 0 {
            let length = left.min(buffer.len() as u64) as usize;
            file.read_exact(&mut buffer[..length]).ok()?;
            digest.update(&buffer[..length]);
            left -= length as u64;
        }
        if digest.clone().finalize()[..] != footer[80..] {
            return None;
        }
        file.set_len(downloaded).ok()?;
        Some(PartialFile {
            path: temporary,
            identity,
            downloaded,
            digest,
            keep_on_exit: true,
        })
    }
}

impl PartialFile {
    pub(in crate::download) fn keep_for_restart(self) {
        if self.keep_on_exit {
            // A failed save leaves no advertised checkpoint.
            let _ = self.persist_checkpoint();
        }
    }

    fn persist_checkpoint(self) -> io::Result<()> {
        let pending = Pending::new(
            self.path.parent().ok_or(io::ErrorKind::InvalidInput)?,
            &self.identity.source,
        );
        check_path(&self.path).map_err(|_| io::ErrorKind::InvalidInput)?;
        check_path(&pending.path).map_err(|_| io::ErrorKind::InvalidInput)?;
        let mut file = OpenOptions::new().append(true).open(&self.path)?;
        if file.metadata()?.len() != self.downloaded {
            return Err(io::ErrorKind::InvalidData.into());
        }
        file.write_all(MAGIC)?;
        file.write_all(&pending.key)?;
        file.write_all(&identity_hash(&self.identity))?;
        file.write_all(&self.identity.length.to_le_bytes())?;
        file.write_all(&self.digest.finalize())?;
        file.sync_all()?;
        drop(file);
        self.path
            .persist_noclobber(&pending.path)
            .map_err(|error| error.error)
    }
}
