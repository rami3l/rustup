# About the team

Following the [call for chartering
work](https://github.com/rust-lang/rfcs/blob/main/text/3037-roadmap-2021.md#chartering-work-in-the-rust-project)
in the Rust 2021 roadmap, below is the charter for the rustup team, i.e. `t-rustup`:

## Mission

The Rustup working group steers the development of `rustup` - a way to securely and reliably install Rust for all Tier 1 systems, as many non-tier 1 systems as possible, for both end users and automated use cases like CI systems.

In addition `rustup` provides many with a low-hanging-fruit contribution vector to get into helping with F/LOSS projects written in Rust, or as a gateway to contributing to the Rust project itself.

## How does this group make decisions?

### Decision maker

- The team as a whole makes decisions, with any stuck decisions down to the [team leads](https://github.com/rust-lang/team/blob/master/teams/wg-rustup.toml#L6)

### Accountable

- Developing the `rustup` Rust installer

### Responsible

- `rustup` and `rustup-init` (the tooling itself)
- How `rustup` is obtained (in conjunction with **t-infra**)
- What `rustup` does, how it does it, its UX

### Consulted

- Changes to the makeup of the rust core distribution packages:
  - size, contents, how files are split up amongst them
  - existence of new components and how to handle them (e.g. if they will need proxying)
  - compression formats
  - changes needed to channel metadata
  - Rust release hosting etc.
- Infrastructure changes affecting the distribution of `rustup` itself.

### Informed

- Changes to system architectures Rust supports, in particular, though not limited to:
  - If architectures are to be promoted, or if they gain tier 2 compiler+cargo support
  - If a platform is to be demoted beneath tier 2 compiler+cargo support
- Changes to how CI cross-build containers are provided (such that `rustup` CI can be updated)
- Changes in licence terms, legal terms, or other topics which impact on `rustup` or installer related web content

## How does this group make decisions?

Small things are decided unilaterally by group members.

Larger things by consensus, with the team leads able to tie break if no consensus can be built.

Where possible, appropriate external entities are consulted and decisions are informed by their input.

## What is expected of members to be part of this group?

1. Some reasonable track record of contribution to the group's work
2. A reasonable understanding of the norms around use cases, users, stability expectations and so on.
3. Preparedness to deal with platforms which may not be the team member's default or preferred.
   For example, a Linux-based developer needs to be prepared to at least work with the Windows-specific aspects of `rustup`.

## Where does this group work?

In [discord](https://discord.gg/YgdaMcEAQy) and [Github](https://github.com/rust-lang/rustup)

## Who is the point of contact for questions on the state of this group?

The team leads. Team membership can be viewed [here](https://www.rust-lang.org/governance/teams/dev-tools#wg-rustup)
