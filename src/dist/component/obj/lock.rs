use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs::{self, File, TryLockError},
    io,
    path::{Path, PathBuf},
};

use anyhow::bail;
use tracing::debug;
use twox_hash::XxHash32;

use crate::{
    config::Cfg,
    install,
    utils::{self, raw},
};

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
        match raw::try_lock(&file) {
            Ok(_) => Ok(Some(ObjLock { _file: file })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Performs the garbage collection of heap objects and in-flight objects/references.
    ///
    /// Garbage collects all heap objects related to the names in `candidates` and removes all
    /// in-flight objects and references left behind by interrupted transactions.
    ///
    /// For the GC of heap objects, if `candidates` is `Some(_)`, then only the objects
    /// corresponding to the names in `candidates` will be considered. Each candidate is essentially
    /// an object address which points to a named directory in `heap/`. When a candidate is found to
    /// be unreachable via any of the existing references, it will be cleaned up. If `candidates` is
    /// `None`, then all existing heap objects will be considered.
    ///
    /// For the GC of in-flight objects and references, they are located under `heap/tmp/`, in a
    /// directory named after the corresponding object's address.
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
        let tmp = &heap.join("tmp");

        let mut locks = HashMap::new();
        if let Some(candidates) = candidates {
            for c in candidates {
                let Some(lock) = self.lock(c)? else {
                    continue;
                };
                locks.insert(Cow::Borrowed(c), lock);
            }
        } else if let Some(dir) = maybe_read_dir("toolchain objects", heap)? {
            for e in dir {
                let Ok(e) = e else {
                    continue;
                };
                if !e.file_type().is_ok_and(|t| t.is_dir()) {
                    continue;
                }
                let f = e.file_name();
                if f == "tmp" {
                    continue;
                }
                let Some(lock) = self.lock(&f)? else {
                    continue;
                };
                locks.insert(Cow::Owned(f), lock);
            }
        };

        if let Some(dir) = maybe_read_dir("in-flight toolchain objects/references", tmp)? {
            for e in dir {
                let Ok(e) = e else {
                    continue;
                };
                if !e.file_type().is_ok_and(|t| !t.is_file()) {
                    continue;
                }
                let obj = e.file_name();

                let maybe_lock = match locks.contains_key(&*obj) {
                    // If the object is already locked for heap GC, there's no need to lock it again.
                    true => None,
                    false if let Some(lock) = self.lock(&obj)? => Some(lock),
                    false => continue,
                };
                debug!(
                    "toolchain address `{}` has stale in-flight objects/references, removing...",
                    obj.display(),
                );
                utils::remove_dir("in-flight object/reference", &tmp.join(obj))?;
                drop(maybe_lock);
            }
        }

        let mut reachable = HashSet::new();
        if let Some(dir) = maybe_read_dir("toolchain references", &cfg.refs_dir)? {
            for entry in dir {
                if let Ok(target) = fs::read_link(entry?.path())
                    && let Some(obj) = target.file_name()
                {
                    reachable.insert(obj.to_owned());
                }
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

fn maybe_read_dir(name: &'static str, path: &Path) -> anyhow::Result<Option<fs::ReadDir>> {
    match utils::read_dir(name, path) {
        Ok(dir) => Ok(Some(dir)),
        Err(e) => match e.downcast_ref::<io::Error>() {
            Some(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            _ => Err(e),
        },
    }
}

#[must_use]
#[clippy::has_significant_drop]
#[derive(Debug)]
pub struct ObjLock {
    _file: File,
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

const LOCKFILE_COUNT: usize = 64;
