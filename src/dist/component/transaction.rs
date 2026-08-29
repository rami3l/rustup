//! A transactional interface to file system operations needed by the
//! installer.
//!
//! Installation or uninstallation of a single component is done
//! within a Transaction, which supports a few simple file system
//! operations. If the Transaction is dropped without committing then
//! it will *attempt* to roll back the transaction.
//!
//! FIXME: This uses ensure_dir_exists in some places but rollback
//! does not remove any dirs created by it.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use tracing::{error, info};

use crate::dist::prefix::{InstallPrefix, InstallPrefixWithOrigin};
use crate::dist::temp;
use crate::errors::RustupError;
use crate::utils;

use super::lock::{ObjLock, ObjLocker};

/// A transaction is responsible for spawning a new toolchain in the toolchain directory.
///
/// The whole process of a transaction is described as follows:
/// - A transaction is created to be associated with a to-be-spawned toolchain object.
/// - A lock is acquired on the object to prevent concurrent modifications.
/// - The in-flight reference and the in-flight object is spawned in the temporary directory.
/// - When the in-flight object is fully spawned, it is moved to the heap directory.
/// - The in-flight reference is moved to the toolchain directory.
/// - The lock is released.
pub struct Transaction {
    /// The [`InstallPrefix`] indicating where the new object should be spawned.
    prefix: InstallPrefix,
    /// The final path of the reference to be spawned.
    ref_: PathBuf,
    lock: Option<ObjLock>,
    tmp_cx: Arc<temp::Context>,
    tmp_obj: temp::Dir,
    tmp_ref: temp::File,
    committed: bool,
    pub(super) permit_copy_rename: bool,
}

impl Transaction {
    pub fn new(
        ref_: PathBuf,
        prefix: InstallPrefixWithOrigin<'_>,
        tmp_cx: Arc<temp::Context>,
        locker: &ObjLocker,
        permit_copy_rename: bool,
    ) -> Result<Self> {
        let ref_name = ref_
            .file_name()
            .context("when extracting base name from reference path")?;
        let obj = prefix
            .dest
            .path()
            .file_name()
            .context("when extracting base name from installation prefix")?;

        let lock = locker.lock(obj)?;

        // TODO: From now on, a full upgrade should never involve removing all its components.
        // Instead, `prefix.orig` should be set straight to `None`. Essentially, a full upgrade will
        // be disqualified from being a "modification".

        let tmp_obj = tmp_cx.new_directory_named(obj)?;
        if let Some(orig) = prefix.orig {
            utils::copy_dir(orig.path(), &tmp_obj)?
        }

        let tmp_ref = tmp_cx.new_file_named(ref_name)?;
        utils::symlink_dir(&Path::new("../toolchains").join(obj), &tmp_ref)?;

        Ok(Self {
            lock: Some(lock),
            tmp_obj,
            tmp_ref,
            prefix: prefix.dest,
            ref_,
            tmp_cx,
            committed: false,
            permit_copy_rename,
        })
    }

    /// Commit must be called for all successful transactions. If not
    /// called the transaction will be rolled back on drop.
    pub fn commit(mut self) -> Result<()> {
        utils::rename(
            "toolchain object",
            &self.tmp_obj,
            self.prefix.path(),
            self.permit_copy_rename,
        )?;
        utils::rename(
            "toolchain reference",
            &self.tmp_ref,
            &self.ref_,
            self.permit_copy_rename,
        )?;
        self.lock.take();
        self.committed = true;
        Ok(())
    }

    /// Add a file at a relative path to the install prefix. Returns a
    /// `File` that may be used to subsequently write the
    /// contents.
    pub fn add_file(&mut self, component: &str, relpath: PathBuf) -> Result<File> {
        let abs_path = self.dest_abs_path(&relpath)?;
        File::create(&abs_path).with_context(|| {
            format!(
                "error creating file '{}' of component '{component}'",
                abs_path.display()
            )
        })
    }

    /// Copy a file to a relative path of the install prefix.
    pub fn copy_file(&mut self, component: &str, relpath: PathBuf, src: &Path) -> Result<()> {
        let abs_path = self.dest_abs_path(&relpath)?;
        utils::copy_file(src, &abs_path).with_context(|| {
            format!(
                "error copying file '{}' of component '{component}'",
                abs_path.display()
            )
        })
    }

    /// Recursively copy a directory to a relative path of the install prefix.
    pub fn copy_dir(&mut self, component: &str, relpath: PathBuf, src: &Path) -> Result<()> {
        let abs_path = self.dest_abs_path(&relpath)?;
        utils::copy_dir(src, &abs_path).with_context(|| {
            format!(
                "error copying directory '{}' of component '{component}'",
                abs_path.display()
            )
        })
    }

    /// Remove a file from a relative path to the install prefix.
    pub fn remove_file(&mut self, component: &str, relpath: PathBuf) -> Result<()> {
        let abs_path = self.dest_abs_path(&relpath)?;
        if !utils::path_exists(&abs_path) {
            return Err(RustupError::ComponentMissingFile {
                name: component.to_owned(),
                path: relpath,
            }
            .into());
        }
        utils::remove_file("component", &abs_path)
    }

    /// Recursively remove a directory from a relative path of the
    /// install prefix.
    pub fn remove_dir(&mut self, component: &str, relpath: PathBuf) -> Result<()> {
        let abs_path = self.dest_abs_path(&relpath)?;
        if !utils::path_exists(&abs_path) {
            return Err(RustupError::ComponentMissingDir {
                name: component.to_owned(),
                path: relpath,
            }
            .into());
        }
        utils::remove_dir("component", &abs_path).with_context(|| {
            format!(
                "error removing directory '{}' of component '{component}'",
                abs_path.display()
            )
        })?;
        Ok(())
    }

    /// Create a new file with string contents at a relative path to
    /// the install prefix.
    pub fn write_file(&mut self, component: &str, relpath: PathBuf, content: String) -> Result<()> {
        let abs_path = self.dest_abs_path(&relpath)?;
        let mut file = File::create(&abs_path).with_context(|| {
            format!(
                "error creating file '{}' of component '{component}'",
                abs_path.display()
            )
        })?;
        utils::write_str("component", &mut file, &abs_path, &content)?;
        Ok(())
    }

    /// Move a file to a relative path of the install prefix.
    pub(crate) fn move_file(
        &mut self,
        component: &str,
        relpath: PathBuf,
        src: &Path,
    ) -> Result<()> {
        let abs_path = self.dest_abs_path(&relpath)?;
        utils::rename("component", src, &abs_path, self.permit_copy_rename).with_context(|| {
            format!(
                "error moving file '{}' of component '{component}'",
                abs_path.display()
            )
        })?;
        Ok(())
    }

    /// Recursively move a directory to a relative path of the install prefix.
    pub(crate) fn move_dir(&mut self, component: &str, relpath: PathBuf, src: &Path) -> Result<()> {
        let abs_path = self.dest_abs_path(&relpath)?;
        utils::rename("component", src, &abs_path, self.permit_copy_rename).with_context(|| {
            format!(
                "error moving directory '{}' of component '{component}'",
                abs_path.display()
            )
        })?;
        Ok(())
    }

    /// Converts a path relative to this [`Transaction`]'s [`InstallPrefix`].
    ///
    /// # Note
    ///
    /// This function is used to map the input relative path to an absolute path in the
    /// corresponding temporary directory. It should be systematically used when an active
    /// transaction is in progress, to avoid writing files directly to the final installation
    /// prefix.
    ///
    /// However, before using this function, please consider whether you can use other methods of
    /// [`InstallPrefix`] that wrap this function instead, as we have provided some convenience
    /// methods for trivial operations to avoid having to call this function directly.
    pub(crate) fn dest_abs_path(&self, relpath: &Path) -> Result<PathBuf> {
        assert!(relpath.is_relative());
        let abs_path = self.tmp_obj.join(relpath);
        if let Some(p) = abs_path.parent() {
            utils::ensure_dir_exists("component", p)?;
        }
        Ok(abs_path)
    }

    pub(crate) fn temp(&self) -> &temp::Context {
        &self.tmp_cx
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        info!(
            "abandoning ongoing transaction targeting '{}'",
            self.prefix.path().display()
        );
        for path in [&*self.tmp_ref, &self.tmp_obj] {
            if let Err(e) = utils::remove_dir("component", path) {
                error!("{e}");
            }
        }
    }
}
