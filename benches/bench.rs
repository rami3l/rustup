#![recursion_limit = "256"]

use std::{cell::RefCell, collections::HashMap, ffi::OsString, fs};

use gungraun::{BinaryBenchmarkConfig, Command, OutputFormat, Stdio, prelude::*};
use rustup::test::{CliTestContext, Scenario};
use tokio::runtime::Runtime;

#[binary_benchmark]
fn proxy_startup_default() -> Command {
    ProxySetup::Default.cmd(&[], &[])
}

#[binary_benchmark]
fn proxy_startup_toml_override() -> Command {
    ProxySetup::TomlOverride.cmd(&[], &[])
}

#[binary_benchmark]
fn proxy_startup_dir_override() -> Command {
    ProxySetup::DirectoryOverride.cmd(&[], &[])
}

#[binary_benchmark]
fn proxy_startup_env() -> Command {
    ProxySetup::Default.cmd(&[], &[("RUSTUP_TOOLCHAIN", "stable")])
}

#[binary_benchmark]
fn proxy_startup_plus_toolchain() -> Command {
    ProxySetup::Default.cmd(&["+stable"], &[])
}

binary_benchmark_group!(
    name = proxy_startup,
    config = BinaryBenchmarkConfig::default()
        .output_format(OutputFormat::default().show_intermediate(true)),
    max_parallel = 1,
    benchmarks = [
        proxy_startup_default,
        proxy_startup_toml_override,
        proxy_startup_dir_override,
        proxy_startup_env,
        proxy_startup_plus_toolchain,
    ]
);

main!(binary_benchmark_groups = proxy_startup);

// HACK: Works around the conflict between gungraun's test harness and rustup's test harness, which
// both assume the ownership of the current working directory. Currently, we disable the former and
// make the latter's test context live until the end of the benchmark harness.
thread_local! {
    static TEST_CONTEXTS: RefCell<Vec<CliTestContext>> = const { RefCell::new(vec![]) };
}

#[derive(Clone, Copy)]
enum ProxySetup {
    Default,
    DirectoryOverride,
    TomlOverride,
}

impl ProxySetup {
    fn cmd(&self, args: &[&str], env_overrides: &[(&str, &str)]) -> Command {
        let runtime = Runtime::new().unwrap();
        let cx = runtime.block_on(CliTestContext::new(Scenario::SimpleV2));

        // Install the stable default toolchain in the test context.
        runtime
            .block_on(cx.config.expect(["rustup", "default", "stable"]))
            .is_ok();

        match self {
            Self::Default => (),
            Self::DirectoryOverride => {
                runtime
                    .block_on(cx.config.expect(["rustup", "override", "set", "stable"]))
                    .is_ok();
            }
            Self::TomlOverride => {
                let toolchain_file = cx.config.current_dir().join("rust-toolchain.toml");
                fs::write(toolchain_file, "[toolchain]\nchannel = \"stable\"\n").unwrap();
            }
        }

        // Reuse the complete environment assembled by rustup's test helpers.
        // Gungraun normally clears the inherited environment, so pass these
        // variables explicitly to reproduce the test command's environment.
        let mut env = HashMap::new();
        cx.config.env(&mut env);
        env.extend(
            env_overrides
                .iter()
                .map(|&(key, value)| (key.to_owned(), value.to_owned())),
        );

        let mock_bin = cx.config.exedir.join("rustc").into_os_string();
        let curr_dir = cx.config.current_dir();

        let mut cmd = Command::new(mock_bin);
        cmd.args(args.iter().map(OsString::from))
            .args(["--echo-env", "CAKE"])
            .envs(env)
            .env("CAKE", "A lie!")
            .current_dir(curr_dir)
            .stdout(Stdio::Null)
            .stderr(Stdio::Null);

        let cmd = cmd.build();
        // Keep the test fixtures alive until the benchmark harness exits.
        TEST_CONTEXTS.with(|contexts| contexts.borrow_mut().push(cx));
        cmd
    }
}
