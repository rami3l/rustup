//! Installation and upgrade of both distribution-managed and local
//! toolchains
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use tracing::debug;

use crate::{
    config::Cfg,
    dist::{
        DistOptions,
        manifest::{Hashed, Manifest},
        prefix::InstallPrefix,
    },
    errors::RustupError,
    toolchain::{CustomToolchainName, LocalToolchainName, Toolchain},
    utils,
};

#[derive(Clone, Debug)]
pub(crate) enum UpdateStatus {
    Installed {
        /// The actual object ID of the installation behind the reference.
        /// If this is an unofficial toolchain, this will be `None`.
        obj: Option<OsString>,
    },
    Updated {
        /// The version of rustc *before* the update.
        from: String,
        /// The actual object ID of the installation behind the reference.
        /// If this is an unofficial toolchain, this will be `None`.
        obj: Option<OsString>,
    },
    Unchanged,
}

impl UpdateStatus {
    pub(crate) fn into_obj(self) -> Option<OsString> {
        match self {
            Self::Installed { obj: Some(obj) } | Self::Updated { obj: Some(obj), .. } => Some(obj),
            _ => None,
        }
    }
}

pub(crate) enum InstallMethod<'cfg, 'a> {
    Link {
        src: &'a Path,
        dest: &'a CustomToolchainName,
        cfg: &'cfg Cfg<'cfg>,
    },
    Dist(DistOptions<'cfg, 'a>),
}

impl InstallMethod<'_, '_> {
    // Install a toolchain
    #[tracing::instrument(level = "trace", err(level = "trace"), skip_all)]
    pub(crate) async fn install(
        self,
        manifest: Option<Hashed<Manifest>>,
    ) -> anyhow::Result<UpdateStatus> {
        // Initialize rayon for use by the remove_dir_all crate limiting the number of threads.
        // This will error if rayon is already initialized but it's fine to ignore that.
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(self.cfg().process.io_thread_count()?.into())
            .build_global();
        let local_name = self.local_name();
        match &self {
            InstallMethod::Link { .. }
            | InstallMethod::Dist(DistOptions {
                old_date_version: None,
                ..
            }) => debug!("installing toolchain {local_name}",),
            _ => debug!("updating existing install for '{local_name}'"),
        }

        let dest_path = self.dest_path();
        debug!("toolchain directory: {}", dest_path.display());
        let status = self.run(&dest_path, manifest).await?;

        match &status {
            UpdateStatus::Unchanged => debug!("toolchain is already up to date"),
            _ => debug!("toolchain {local_name} installed"),
        };

        // Final check, to ensure we're installed
        match Toolchain::exists(self.cfg(), &local_name)? {
            true => Ok(status),
            false => Err(RustupError::ToolchainNotInstallable(local_name.to_string()).into()),
        }
    }

    async fn run(
        &self,
        path: &Path,
        manifest: Option<Hashed<Manifest>>,
    ) -> anyhow::Result<UpdateStatus> {
        if path.exists() {
            // Don't uninstall first for Dist method
            match self {
                InstallMethod::Dist { .. } => {}
                _ => {
                    uninstall(path)?;
                }
            }
        }

        Ok(match self {
            InstallMethod::Link { src, .. } => {
                utils::symlink_dir(src, path)?;
                UpdateStatus::Installed { obj: None }
            }
            InstallMethod::Dist(opts) => match opts
                .install_into(&InstallPrefix::from(path.to_owned()), manifest)
                .await?
            {
                None => UpdateStatus::Unchanged,
                Some(Hashed { inner: obj, hash }) => {
                    utils::write_file("update hash", &opts.update_hash, &hash)?;
                    match opts {
                        DistOptions {
                            old_date_version: Some((_, v)),
                            ..
                        } => UpdateStatus::Updated {
                            obj,
                            from: v.clone(),
                        },
                        _ => UpdateStatus::Installed { obj },
                    }
                }
            },
        })
    }

    fn cfg(&self) -> &Cfg<'_> {
        match self {
            InstallMethod::Link { cfg, .. } => cfg,
            InstallMethod::Dist(DistOptions { cfg, .. }) => cfg,
        }
    }

    fn local_name(&self) -> LocalToolchainName {
        match self {
            InstallMethod::Link { dest, .. } => (*dest).clone().into(),
            InstallMethod::Dist(DistOptions {
                toolchain: desc, ..
            }) => (*desc).clone().into(),
        }
    }

    fn dest_path(&self) -> PathBuf {
        self.cfg().ref_path(&self.local_name())
    }
}

pub(crate) fn uninstall(path: &Path) -> anyhow::Result<()> {
    utils::remove_dir("install", path)
}
