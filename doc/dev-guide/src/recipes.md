# Recipes

This section contains some recipes for common tasks that you may want to
perform when contributing to rustup.

## Supporting a new compilation target

Adding support for a new compilation target involves the following steps:

1. Informing rustup of the new target:

   Since you are here, it is quite likely that you are promoting a target from
   tier 3 to tier 2. As such, it is often sufficient to just cross-compile
   rustup from Linux to that target.

   You can refer to [rustup#4688] for a practical example for this step. You
   can find in that PR nearly all places where you would need to mention your
   new target in the rustup codebase.

   Notably, you would need to add a line in
   `ci/actions-templates/linux-builds-template.yaml` to include it in rustup's
   CI, while disabling the build for this target in all scenarios. At the
   moment of writing, this is done by appending the YAML comment
   `# skip-pr skip-master skip-stable`
   at the end of the line when mentioning your target in that file.

   [rustup#4688]: https://github.com/rust-lang/rustup/pull/4688

2. Stabilizing the target:

   When your new target reaches stable Rust, you can then enable the target in
   certain CI scenarios, depending on the popularity of the target. At the
   moment of writing, this is done by removing certain occurrences of `skip-*`
   in the aforementioned YAML comment. In most cases, you would only need to
   enable the target for the `stable` CI scenario by removing `skip-stable`.

   You can refer to [rustup#4816] for a practical example for this step.

   Do note that when creating the PR for this step, you will need to prove that
   the target's CI is indeed working by removing `skip-pr` in a separate commit
   to temporarily enable this target in this PR's CI. Once the CI is green, you
   can send the link ([example][send-link]) to that CI run in the PR thread for
   verification. After that, you can safely drop the temporary commit to get
   the PR ready for merging.

   [rustup#4816]: https://github.com/rust-lang/rustup/pull/4816
   [send-link]: https://github.com/rust-lang/rustup/pull/4816#issuecomment-4263419604
