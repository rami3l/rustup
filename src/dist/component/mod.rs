/// An interpreter for the rust-installer [1] installation format.
///
/// https://github.com/rust-lang/rust-installer
pub use self::transaction::*;
pub use self::{components::*, lock::*, obj::*, package::*};

// Transactional file system tools
mod transaction;
// The representation of a package, its components, and installation
mod package;
// The representation of *installed* components, and uninstallation
mod components;
// The representation of an object and its identification-related semantics.
mod obj;
// The per-object FS locks.
mod lock;

#[cfg(test)]
mod tests;
