mod hash;
mod lock;

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs,
};

pub use hash::HashEncoder;
use itertools::Either;
pub use lock::{ObjLock, ObjLocker};
use tracing::{debug, info};

use crate::{config::Cfg, install, utils};

/// Garbage collects all objects in the collection `candidates`.
///
/// Each candidate is identified with the object ID which points to a named directory in the heap.
/// When a candidate is found to be unreachable via any of the existing references, it will be
/// cleaned up. If `candidates` is `None`, then it defaults to all existing objects.
pub(crate) fn gc<'a>(
    candidates: Option<impl IntoIterator<Item = &'a OsStr>>,
    locker: &ObjLocker,
    cfg: &Cfg<'_>,
) -> anyhow::Result<()> {
    let candidates: Option<HashSet<_>> = candidates.map(HashSet::from_iter);
    if candidates.as_ref().is_some_and(HashSet::is_empty) {
        return Ok(());
    }

    let heap = &cfg.toolchains_dir;
    let mut locks = match candidates {
        Some(candidates) => Either::Left(candidates.into_iter().map(Cow::Borrowed)),
        None => Either::Right(utils::read_dir("toolchain objects", heap)?.filter_map(|e| {
            let e = e.ok()?;
            if !e.file_type().is_ok_and(|t| t.is_dir()) {
                return None;
            }
            match e.file_name() {
                f if f == "tmp" => None,
                f => Some(Cow::Owned(f)),
            }
        })),
    }
    .filter_map(|c| match locker.lock(&*c) {
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
        debug!(
            "toolchain object `{}` is reachable and thus retained",
            obj.display(),
        );
        locks.remove(obj.as_os_str());
    }
    for (obj, lock) in locks {
        info!(
            "toolchain object `{}` is unreachable, removing...",
            obj.display(),
        );
        install::uninstall(&heap.join(&*obj))?;
        drop(lock);
    }
    Ok(())
}
