use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs::{self, File, TryLockError},
    io,
    path::{Path, PathBuf},
};

use anyhow::bail;
use itertools::Either;
use tracing::debug;
use twox_hash::XxHash32;

use crate::{config::Cfg, install, utils};

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

    /// Garbage collects all objects in the collection `candidates`.
    ///
    /// Each candidate is identified with the object ID which points to a named directory in the heap.
    /// When a candidate is found to be unreachable via any of the existing references, it will be
    /// cleaned up. If `candidates` is `None`, then it defaults to all existing objects.
    pub(crate) fn gc<'a>(
        &self,
        candidates: Option<impl IntoIterator<Item = &'a OsStr>>,
        cfg: &Cfg<'_>,
    ) -> anyhow::Result<()> {
        let candidates: Option<HashSet<_>> = candidates.map(HashSet::from_iter);
        if candidates.as_ref().is_some_and(HashSet::is_empty) {
            return Ok(());
        }

        let heap = &cfg.toolchains_dir;
        let mut locks = match candidates {
            Some(candidates) => Either::Left(candidates.into_iter().map(Cow::Borrowed)),
            None => match utils::read_dir("toolchain objects", heap) {
                Err(e) => {
                    return match e.downcast_ref::<io::Error>() {
                        Some(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                        _ => Err(e),
                    };
                }
                Ok(dir) => Either::Right(dir.filter_map(|e| {
                    let e = e.ok()?;
                    if !e.file_type().is_ok_and(|t| t.is_dir()) {
                        return None;
                    }
                    match e.file_name() {
                        f if f == "tmp" => None,
                        f => Some(Cow::Owned(f)),
                    }
                })),
            },
        }
        .filter_map(|c| match self.lock(&*c) {
            Ok(Some(lock)) => Some(Ok((c, lock))),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        })
        .collect::<anyhow::Result<HashMap<_, _>>>()?;

        let mut reachable = HashSet::new();
        let walker = cfg.refs_dir.read_dir()?;
        for entry in walker {
            // TODO: Consider junctions on Windows
            if let Ok(target) = fs::read_link(entry?.path())
                && let Some(obj) = target.file_name()
            {
                reachable.insert(obj.to_owned());
            }
        }

        for obj in &reachable {
            if locks.remove(obj.as_os_str()).is_some() {
                debug!(
                    "toolchain object `{}` is reachable during GC, skipping its removal...",
                    obj.display(),
                );
            }
        }
        for (obj, lock) in locks {
            debug!(
                "toolchain object `{}` is unreachable during GC, removing...",
                obj.display(),
            );
            install::uninstall(&heap.join(&*obj))?;
            drop(lock);
        }
        Ok(())
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
