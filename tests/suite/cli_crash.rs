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

// TODO: A modification killed in the middle leaves files behind that the next attempt trips over,
// since the metadata that would have accounted for them was never written.
//
// Unlike [`interrupted_install_can_be_retried()`], there is no "partially installed toolchain"
// recovery path for components, so the toolchain stays stuck.
//
// See: <https://github.com/rust-lang/rustup/issues/4400>
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

// TODO: `rustup component add` races against another `rustup component add` of the same component.
//
// The second process cannot tell that the first one is already installing the component, so it
// tries to install it again on top of the first one's files.
//
// See: <https://github.com/rust-lang/rustup/issues/3716>
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

// TODO: This failure is similar to [`concurrent_component_add_conflicts()`], but on a component
// that provides a proxy.
//
// See: <https://github.com/rust-lang/rustup/issues/4465>
#[tokio::test]
async fn concurrent_component_add_conflicts_on_proxy_call() {
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

// TODO: A proxy launched while a modification is in flight. During this time, it sees a toolchain
// whose files and metadata disagree.
//
// In this test, the `cargo` files are already gone, but the toolchain is still advertised as
// having the `cargo` component, hence the unhelpful message.
//
// See: <https://github.com/rust-lang/rust-clippy/issues/12763>
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
error: the 'cargo[EXE]' binary, normally provided by the 'cargo' component, is not applicable to the 'nightly-[HOST_TUPLE]' toolchain

"#]])
        .is_err();

    assert!(parked.resume().success());
    cx.config
        .expect(["cargo", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: 'cargo[EXE]' is not installed for the toolchain 'nightly-[HOST_TUPLE]'.
...
"#]])
        .is_err();
}

// TODO: When uninstalling a toolchain while it is being installed, the process installing the
// toolchain is not aware of the race, so it happily considers a toolchain directory that lacks most
// of its files as fully installed.
//
// See: <https://github.com/rust-lang/rustup/issues/988#issuecomment-593632709>
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
error: the 'rustc[EXE]' binary, normally provided by the 'rustc' component, is not applicable to the 'nightly-[HOST_TUPLE]' toolchain

"#]])
        .is_err();
}
