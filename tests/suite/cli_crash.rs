use rustup::{
    dist::{
        download::CHECKPOINT_DOWNLOAD_BEFORE_RENAME,
        manifestation::CHECKPOINT_UPDATE_BEFORE_METADATA,
    },
    test::{CliTestContext, Scenario},
};

#[tokio::test]
async fn interrupted_install_can_be_retried() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.kill_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "toolchain", "install", "nightly"],
    )
    .is_err();

    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "+nightly", "--version"])
        .await
        .is_ok();
}

#[tokio::test]
async fn interrupted_install_cannot_be_cleaned_up() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.kill_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "toolchain", "install", "nightly"],
    )
    .is_err();

    let tmp = &cx.config.rustupdir.join("heap/tmp");
    assert_eq!(tmp.read_dir().unwrap().count(), 1);

    cx.config
        .expect(["rustup", "update"])
        .await
        .with_stderr(snapbox::str![[r#"
info: no updatable toolchains installed
info: cleaning up downloads & tmp directories
warn: no toolchain installed and no default toolchain set
...
"#]])
        .is_ok();
    assert_eq!(tmp.read_dir().unwrap().count(), 1);
}

/// Two processes installing the same toolchain shouldn't race against each other.
///
/// See: <https://github.com/rust-lang/rustup/issues/4607>
#[tokio::test]
async fn concurrent_install_bails_out() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;

    let _parked = cx.spawn_at(
        CHECKPOINT_DOWNLOAD_BEFORE_RENAME,
        ["rustup", "toolchain", "install", "nightly"],
    );
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for nightly-[HOST_TUPLE]
info: latest update on 2015-01-02 for version 1.3.0 (hash-nightly-2)
error: object `[..]` is already locked by another transaction

"#]])
        .is_err();
}

/// When a proxy launches while a modification is in flight, it shouldn't see a toolchain in its
/// transient state whose files and metadata disagree.
///
/// See: <https://github.com/rust-lang/rust-clippy/issues/12763>
#[tokio::test]
async fn proxy_during_component_removal_sees_consistent_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let parked = cx.spawn_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "component", "remove", "cargo"],
    );
    cx.config
        .expect(["cargo", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();

    parked.resume().is_ok();
    cx.config
        .expect(["cargo", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: 'cargo[EXE]' is not installed for the toolchain 'nightly-[HOST_TUPLE]'.
...
"#]])
        .is_err();
}

/// Uninstalling a toolchain while it is being installed shouldn't cause a broken toolchain to be
/// left behind.
///
/// See: <https://github.com/rust-lang/rustup/issues/988#issuecomment-593632709>
#[tokio::test]
async fn uninstall_during_install_leaves_good_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;

    let parked = cx.spawn_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "toolchain", "install", "nightly"],
    );
    cx.config
        .expect(["rustup", "toolchain", "uninstall", "nightly"])
        .await
        .is_ok();
    parked.resume().is_ok();

    cx.config
        .expect(["rustup", "toolchain", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
nightly-[HOST_TUPLE] (active, default)

"#]])
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

/// A modification killed in the middle shouldn't leave files that the next attempt can trip over.
///
/// See: <https://github.com/rust-lang/rustup/issues/4400>
#[tokio::test]
async fn interrupted_component_add_can_be_retried() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    cx.kill_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "component", "add", "rust-src"],
    )
    .is_err();

    cx.config
        .expect(["rustup", "component", "add", "rust-src"])
        .await
        .with_stderr(snapbox::str![[r#"
info: downloading component rust-src

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "component", "list", "--installed"])
        .await
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TUPLE]
rust-docs-[HOST_TUPLE]
rust-src
rust-std-[HOST_TUPLE]
rustc-[HOST_TUPLE]

"#]])
        .is_ok();
}

/// `rustup component add` should not race against another `rustup component add` of the same component.
///
/// See: <https://github.com/rust-lang/rustup/issues/3716>
#[tokio::test]
async fn concurrent_component_add_same_component_bails_out() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let parked = cx.spawn_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "component", "add", "rust-src"],
    );
    cx.config
        .expect(["rustup", "component", "add", "rust-src"])
        .await
        .with_stderr(snapbox::str![[r#"
error: object `[..]` is already locked by another transaction

"#]])
        .is_err();

    parked.resume().is_ok();
}

/// This test is similar to [`concurrent_component_add_same_component_bails_out()`], but on a
/// component that provides a binary in `/bin`.
///
/// See: <https://github.com/rust-lang/rustup/issues/4465>
#[tokio::test]
async fn concurrent_component_add_same_component_bails_out_with_bin() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "remove", "cargo"])
        .await
        .is_ok();

    let parked = cx.spawn_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "component", "add", "cargo"],
    );
    cx.config
        .expect(["rustup", "component", "add", "cargo"])
        .await
        .with_stderr(snapbox::str![[r#"
error: object `[..]` is already locked by another transaction

"#]])
        .is_err();

    parked.resume().is_ok();
}

/// Two `rustup component add` of *different* components shouldn't corrupt the global state of the
/// toolchain.
///
/// See: <https://github.com/rust-lang/rustup/issues/3530>
#[tokio::test]
async fn concurrent_component_add_different_components_bails_out() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let parked = cx.spawn_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "component", "add", "rust-src"],
    );
    cx.config
        .expect(["rustup", "component", "add", "rust-analysis"])
        .await
        .with_stderr(snapbox::str![[r#"
error: object `[..]` is already locked by another transaction

"#]])
        .is_err();
    parked.resume().is_ok();

    cx.config
        .expect(["rustup", "component", "list", "--installed"])
        .await
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TUPLE]
rust-docs-[HOST_TUPLE]
rust-src
rust-std-[HOST_TUPLE]
rustc-[HOST_TUPLE]

"#]])
        .is_ok();
}

/// A component removal killed in the middle should have no visible effect on the toolchain.
///
/// See: <https://github.com/rust-lang/rustup/issues/2417>
#[tokio::test]
async fn interrupted_component_remove_remains_undone() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    cx.kill_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "component", "remove", "cargo"],
    )
    .is_err();

    cx.config
        .expect(["cargo", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "component", "list", "--installed"])
        .await
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TUPLE]
...
"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "component", "add", "cargo"])
        .await
        .with_stderr(snapbox::str![[r#"
info: component cargo is up to date

"#]])
        .is_ok();
    cx.config.expect(["cargo", "--version"]).await.is_ok();

    cx.config
        .expect(["rustup", "component", "remove", "cargo"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "add", "cargo"])
        .await
        .is_ok();
    cx.config.expect(["cargo", "--version"]).await.is_ok();
}
