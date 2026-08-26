use std::{
    ffi::OsStr,
    fs::{File, TryLockError},
    path::{Path, PathBuf},
};

use anyhow::bail;

use super::HashEncoder;
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
    /// The object ID should be valid Unicode, with three parts separated `-`, and the bytes of the
    /// two latter parts should fall into [`HashEncoder::ALPHABET`]. Otherwise, this function will
    /// return `None`.
    fn lock_name(obj: &OsStr) -> Option<String> {
        let alphabet = HashEncoder::ALPHABET.as_ref();
        let obj = obj.to_str()?;
        let [snd, fst, _] = *obj.rsplitn(3, '-').collect::<Vec<_>>() else {
            return None;
        };
        let to_digits = |s| base_x::decode(alphabet, s).ok();
        let lock_id = to_digits(snd)?
            .into_iter()
            .chain(to_digits(fst)?)
            .fold(0_usize, |acc, it| {
                acc.carrying_mul_add(alphabet.len(), 0, it.into()).0
            });
        // Take modulo of the resulting number to avoid creating too many lockfiles.
        let lock_id = lock_id % LOCKFILE_COUNT;
        Some(format!("{lock_id:x}.lock"))
    }
}

impl Drop for ObjLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

const LOCKFILE_COUNT: usize = 64;
