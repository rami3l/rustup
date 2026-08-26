use std::{
    ffi::OsStr,
    fs::File,
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};

use crate::utils;

use super::obj::HashEncoder;

pub struct ObjLocker {
    dir: PathBuf,
}

impl ObjLocker {
    pub fn new(dir: &Path) -> Result<Self> {
        utils::ensure_dir_exists("lock directory", dir)?;
        Ok(Self {
            dir: dir.to_owned(),
        })
    }

    pub fn lock(&self, obj: &OsStr) -> Result<ObjLock> {
        let Some(lock) = ObjLock::lock_name(obj) else {
            bail!("invalid object ID `{}`", obj.display());
        };

        let file = File::create(self.dir.join(lock))?;
        file.try_lock()?;
        Ok(ObjLock { file })
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
    /// The object ID should be valid Unicode, with two parts separated by the last `-`, and the
    /// last byte of each part should fall into [`HashEncoder::ALPHABET`]. Otherwise, this function
    /// will return `None`.
    fn lock_name(obj: &OsStr) -> Option<String> {
        let alphabet = HashEncoder::ALPHABET;
        let obj = obj.to_str()?;
        let (fst, snd) = obj.rsplit_once('-')?;
        let to_digit = |c: &u8| alphabet.iter().position(|it| it == c);
        let lock_id =
            to_digit(fst.as_bytes().last()?)? * alphabet.len() + to_digit(snd.as_bytes().last()?)?;
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
