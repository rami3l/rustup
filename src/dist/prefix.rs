use std::path::{Path, PathBuf};

use crate::utils;

use super::manifestation::Changes;

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
pub struct InstallPrefixWithOrigin<'a> {
    pub dest: InstallPrefix,
    pub orig: Option<&'a InstallPrefix>,
}

impl<'a> InstallPrefixWithOrigin<'a> {
    /// Generates a new installation prefix with the given original prefix and changes.
    ///
    /// # Note
    ///
    /// In the "process safe rustup" proposal, we assume that the install prefix should have
    /// a base name that looks like
    /// `<readable-short-name>-<xxhash-rustc-ver>-<xxhash-component-list>`, with `xxhash*`es
    /// falling into [`HashEncoder::ALPHABET`].
    ///
    /// For the first stage where A/B partitioning is used, we change the address format to
    /// one of the following:
    ///
    /// ```
    /// <ref-short-name>-abpart1t10ned-a
    /// <ref-short-name>-abpart1t10ned-b
    /// ```
    ///
    /// ... where `ref-short-name` looks like `stableaarch64appledarwin`
    ///
    /// When flipping the active partition, if the original prefix base name doesn't match the above
    /// format, we consider the current active partition to be `a`.
    pub fn new(orig: &'a InstallPrefix, changes: &Changes) -> Self {
        if changes.is_empty() {
            return Self {
                orig: Some(orig),
                dest: orig.clone(),
            };
        }
        let dest = todo!("read the old prefix and decide");
        Self {
            orig: Some(orig),
            dest,
        }
    }
}

impl From<InstallPrefix> for InstallPrefixWithOrigin<'_> {
    fn from(prefix: InstallPrefix) -> Self {
        Self {
            dest: prefix,
            orig: None,
        }
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
