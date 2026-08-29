use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;

use crate::utils;

/// The relative path to the manifest directory in a Rust installation,
/// with path components separated by [`std::path::MAIN_SEPARATOR`].
const REL_MANIFEST_DIR: &str = match std::path::MAIN_SEPARATOR {
    '/' => "lib/rustlib",
    '\\' => r"lib\rustlib",
    _ => panic!("unknown `std::path::MAIN_SEPARATOR`"),
};

static V1_COMMON_COMPONENT_LIST: &[&str] = &["cargo", "rustc", "rust-docs"];
pub(crate) const DIST_MANIFEST: &str = "multirust-channel-manifest.toml";

/// Describes the target of an installation.
///
/// This struct is composed of a final destination path and an original source path. When the
/// installation is a modification of an existing installation, the origin source path corresponds
/// to the path of that installation. Otherwise, the origin source path is `None`.
#[derive(Clone, Debug)]
pub struct InstallPrefixWithOrigin {
    pub dest: InstallPrefix,
    pub orig: Option<InstallPrefix>,
}

pub trait AddressingStrategy {
    fn address(&self, reference: &InstallPrefix) -> Result<InstallPrefixWithOrigin>;
}

#[derive(Clone, Debug)]
pub struct AbAddressing {
    heap: PathBuf,
}

impl AbAddressing {
    pub fn new(heap: PathBuf) -> Self {
        Self { heap }
    }

    fn partition(reference: &InstallPrefix) -> Option<&'static str> {
        let target = fs::read_link(reference.path()).ok()?;
        let name = target.file_name()?.to_str()?;
        if name.ends_with("-A") {
            Some("A")
        } else if name.ends_with("-B") {
            Some("B")
        } else {
            None
        }
    }
}

impl AddressingStrategy for AbAddressing {
    fn address(&self, reference: &InstallPrefix) -> Result<InstallPrefixWithOrigin> {
        let name = reference
            .path()
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("reference path has no file name"))?;
        let name = name
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("reference path is not valid Unicode"))?;
        let partition = match Self::partition(reference) {
            Some("A") => "B",
            Some("B") | None => "A",
            Some(_) => "A",
        };
        let dest = self.heap.join(format!("{name}-{partition}"));
        utils::ensure_dir_exists("heap", &self.heap)?;
        utils::ensure_dir_exists(
            "toolchain reference",
            reference
                .path()
                .parent()
                .ok_or_else(|| anyhow::anyhow!("reference path has no parent"))?,
        )?;

        Ok(InstallPrefixWithOrigin {
            dest: InstallPrefix::from(dest),
            orig: reference.path().exists().then(|| reference.clone()),
        })
    }
}

#[derive(Clone, Debug)]
pub struct InstallPrefix {
    path: PathBuf,
}

impl InstallPrefix {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn abs_path<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.path.join(path)
    }

    pub(crate) fn manifest_dir(&self) -> PathBuf {
        let mut path = self.path.clone();
        path.push(REL_MANIFEST_DIR);
        path
    }

    pub fn manifest_file(&self, name: &str) -> PathBuf {
        let mut path = self.manifest_dir();
        path.push(name);
        path
    }

    pub(crate) fn dist_manifest(&self) -> Option<PathBuf> {
        let path = self.manifest_file(DIST_MANIFEST);
        utils::path_exists(&path).then_some(path)
    }

    pub(crate) fn rel_manifest_file(&self, name: &str) -> PathBuf {
        let mut path = PathBuf::from(REL_MANIFEST_DIR);
        path.push(name);
        path
    }

    /// Guess whether this is a V1 or V2 manifest distribution.
    pub(crate) fn guess_v1_manifest(&self) -> bool {
        // If all the v1 common components are present this is likely to be
        // a v1 manifest install.  The v1 components are not called the same
        // in a v2 install.
        for component in V1_COMMON_COMPONENT_LIST {
            let manifest = format!("manifest-{component}");
            let manifest_path = self.manifest_file(&manifest);
            if !utils::path_exists(manifest_path) {
                return false;
            }
        }
        // It's reasonable to assume this is a v1 manifest installation
        true
    }
}

impl From<&Path> for InstallPrefix {
    fn from(value: &Path) -> Self {
        Self { path: value.into() }
    }
}

impl From<PathBuf> for InstallPrefix {
    fn from(path: PathBuf) -> Self {
        Self { path }
    }
}
