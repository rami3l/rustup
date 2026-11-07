use rustup::{
    dist::manifestation::CHECKPOINT_UPDATE_BEFORE_METADATA,
    test::{CliTestContext, Scenario},
};

#[tokio::test]
async fn interrupted_install_can_be_retried() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let status = cx.kill_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "toolchain", "install", "nightly"],
    );
    assert!(!status.success());

    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "+nightly", "--version"])
        .await
        .is_ok();
}

// The tests below exercise the different symptoms of
// https://github.com/rust-lang/rustup/issues/988: rustup has no defense against
// another rustup process (or an interrupted one) working on the same toolchain.
//
// They all park a rustup process at `CHECKPOINT_UPDATE_BEFORE_METADATA`, i.e. right after the
// components have been written into the toolchain directory but before the
// `rustup-dist.toml` and `rustup-config.toml` files recording them have been written. This
// is the same window in which a concurrent process (or a crash) would leave the toolchain in
// a state that no other rustup process can make sense of.
//
// Each test asserts the *current* (undesired) behavior and should be flipped once
// toolchain modifications are made safe against concurrency.

/// `rustup component add` racing against another `rustup component add` of the same component.
///
/// The second process cannot tell that the first one is already installing the component, so it
/// tries to install it again on top of the first one's files.
///
/// See: <https://github.com/rust-lang/rustup/issues/3716>
#[tokio::test]
async fn concurrent_component_add_conflicts() {
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
...
error: failed to install component: 'rust-src', detected conflict: 'lib/rustlib/src/rust-src/foo.rs'
...
"#]])
        .is_err();

    assert!(parked.resume().success());
}

/// The same race as above, but on a component that provides a proxied binary.
///
/// This is the closest we can get to the `detected conflict: 'bin/cargo'` of
/// <https://github.com/rust-lang/rustup/issues/4465>: the checkpoint can only be hit after
/// all the files of a component have been written, so the component is re-added instead.
#[tokio::test]
async fn concurrent_component_add_conflicts_on_proxied_binary() {
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
...
error: failed to install component: 'cargo-[HOST_TUPLE]', detected conflict: 'bin/cargo[..]'
...
"#]])
        .is_err();

    assert!(parked.resume().success());
}

/// A proxy launched while a modification is in flight sees a toolchain whose files and
/// metadata disagree.
///
/// Here the `cargo` files are already gone, but the toolchain is still advertised as having the
/// `cargo` component, hence the unhelpful message (compare with the one after the commit, which
/// suggests `rustup component add cargo`).
///
/// See: <https://github.com/rust-lang/rust-clippy/issues/12763>
#[tokio::test]
async fn proxy_during_component_removal_sees_inconsistent_toolchain() {
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
        .with_stderr(snapbox::str![[r#"
error: the 'cargo' binary, normally provided by the 'cargo' component, is not applicable to the 'nightly-[HOST_TUPLE]' toolchain

"#]])
        .is_err();

    assert!(parked.resume().success());
    cx.config
        .expect(["cargo", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: 'cargo' is not installed for the toolchain 'nightly-[HOST_TUPLE]'.
...
"#]])
        .is_err();
}

/// Uninstalling a toolchain while it is being installed is not detected by the installer,
/// which then happily resurrects a toolchain directory that lacks most of its files.
///
/// See: <https://github.com/rust-lang/rustup/issues/988> (the report from the Libra team).
#[tokio::test]
async fn uninstall_during_install_leaves_broken_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;

    let parked = cx.spawn_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "toolchain", "install", "nightly"],
    );
    cx.config
        .expect(["rustup", "toolchain", "uninstall", "nightly"])
        .await
        .is_ok();
    assert!(parked.resume().success());

    // The toolchain shows up as installed...
    cx.config
        .expect(["rustup", "toolchain", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
nightly-[HOST_TUPLE] (active, default)

"#]])
        .is_ok();
    // ... but it is not usable.
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: the 'rustc' binary, normally provided by the 'rustc' component, is not applicable to the 'nightly-[HOST_TUPLE]' toolchain

"#]])
        .is_err();
}

/// A modification killed in the middle leaves files behind that the next attempt trips over,
/// since the metadata that would have accounted for them was never written.
///
/// Unlike `interrupted_install_can_be_retried`, there is no "partially installed toolchain"
/// recovery path for components, so the toolchain stays stuck.
///
/// See: <https://github.com/rust-lang/rustup/issues/4400>
#[tokio::test]
async fn interrupted_component_add_cannot_be_retried() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let status = cx.kill_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "component", "add", "rust-src"],
    );
    assert!(!status.success());

    cx.config
        .expect(["rustup", "component", "add", "rust-src"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: failed to install component: 'rust-src', detected conflict: 'lib/rustlib/src/rust-src/foo.rs'
...
"#]])
        .is_err();
}
