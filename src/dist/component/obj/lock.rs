use std::{
    ffi::OsStr,
    fs::{File, TryLockError},
    path::{Path, PathBuf},
};

use anyhow::bail;
use twox_hash::XxHash32;

use crate::utils;

pub struct ObjLocker {
    dir: PathBuf,
}

impl ObjLocker {
    pub fn new(dir: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            dir: dir.to_owned(),
        })
    }

    pub fn lock(&self, obj: impl AsRef<OsStr>) -> anyhow::Result<Option<ObjLock>> {
        let obj = obj.as_ref();
        let Some(lock) = ObjLock::lock_name(obj) else {
            bail!("invalid object ID `{}`", obj.display());
        };

        utils::ensure_dir_exists("lock directory", &self.dir)?;
        let file = File::create(self.dir.join(lock))?;
        match file.try_lock() {
            Ok(_) => Ok(Some(ObjLock { file })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

#[must_use]
#[clippy::has_significant_drop]
#[derive(Debug)]
pub struct ObjLock {
    file: File,
}

impl ObjLock {
    /// Generates a lock name from the given object ID string.
    ///
    /// The object ID should be valid Unicode, with at least two parts separated by `-`.
    /// Otherwise, this function will return `None`.
    fn lock_name(obj: &OsStr) -> Option<String> {
        let obj = obj.to_str()?;
        let (_prefix, rest) = obj.split_once('-')?;

        const SEED: u32 = 0x1ced_7ea5;
        let lock_id = XxHash32::oneshot(SEED, rest.as_bytes()) as usize;
        // Take modulo of the resulting number to avoid creating too many lockfiles.
        let lock_id = lock_id % LOCKFILE_COUNT;
        Some(format!("{lock_id:02x}.lock"))
    }
}

impl Drop for ObjLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

const LOCKFILE_COUNT: usize = 64;
