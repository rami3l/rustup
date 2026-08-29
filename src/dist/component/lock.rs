use std::{
    ffi::OsStr,
    fs::File,
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};

use crate::utils;

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
    /// Generates a lock name from the reference or object ID.
    fn lock_name(obj: &OsStr) -> Option<String> {
        let obj = obj.to_str()?;
        let lock_id = obj.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        }) as usize
            % LOCKFILE_COUNT;
        Some(format!("{lock_id:x}.lock"))
    }
}

impl Drop for ObjLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

const LOCKFILE_COUNT: usize = 64;
