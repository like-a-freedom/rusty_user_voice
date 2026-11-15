# ADR-0008: The agent-facing skill corpus is a build artifact of the CLI binary

Status: proposed

## Context

`uservoice-cli` and `uservoice-mcp` expose a wide contract: seven MCP tools, a CLI with 68 long flags across seven subcommands, a two-phase mutation protocol with a 15-minute preview TTL, and a stable error vocabulary of five reason tokens and four envelope statuses. An agent that wants to use this tool has to know all of it.

The repository shipped one skill, `skills/uservoice-pm/`, as hand-maintained Markdown. It had drifted. It described the MCP tools and omitted the CLI, it predated `login`, and no check compared it against the code. Nothing failed when a flag was renamed or a tool description changed.

The reference implementations solve this by serving the content from the binary. `agent-browser skills get core` returns the guide for the installed version, so the instructions cannot describe a version that does not exist. `gcx agent skills get` does the same. `keenable` takes the opposite approach and ships a static skill, which is exactly the shape that goes stale.

We want the same guarantee: the text an agent reads always matches the binary it is running.

## Decision

The skill corpus moves into the crate at `crates/rusty_user_voice/skills/`. `build.rs` discovers the files and generates a table of path and content. `uservoice-cli skills` serves that table.

```text
uservoice-cli skills list
uservoice-cli skills get <name> [--full] [--ref <path>] [--all]
```

The global `--output` decides the shape. `human` prints the document as it is, which is what an agent wants to read. `json` wraps it, which is what a script wants. The command has no `--json` of its own: the output mode is the repo-wide rule, and a second flag for the same thing would be a second way to ask the same question.

`skills/uservoice-pm/SKILL.md` stays at the repository root as a stub that points at the command.

Two properties of the placement were verified rather than assumed:

- An `include_str!` reaching outside the package root compiles under `cargo build` and under `cargo install --path`.
- The same `include_str!` breaks `cargo package`. The tarball omits the out-of-package file, and the extracted crate fails to build.

Inside the crate, packaging, installing, and vendoring all carry the corpus with the code.

The corpus is split by progressive disclosure. `core` is the always-loaded layer: the read loop, the write loop, safety rules, the error vocabulary, and a map of what to load next. `triage` and `feedback-loop` are separate tasks. `cli`, `mcp-tools`, `pagination`, `errors`, and `envelope` are references, one loaded at a time.

The corpus is verified against the code on every CI run. Flags are reflected out of the clap tree, tool names come from `ToolName::ALL`, statuses from `OutcomeStatus`, reason tokens from named constants. A change that moves the contract without moving the corpus fails the build.

Verification is not limited to the surface. The corpus may only claim what the code guarantees. One claim was wrong: "a Change ID is spent once, and the ledger rejects a second apply". The ledger keys idempotency on the Idempotency Key, and a Change ID alone stops nothing. A second apply under a different key runs again unless the freshness check catches it, which it does only because the first write moved the Idea. The corpus says that instead, and `tests/app_mutation.rs` pins the behavior.

`skills` is intercepted before `Settings::from_sources`, because settings load fails without credentials and the guide has to be readable before login.

## Rejected

**Keep the corpus in the repository root and read it at runtime.** The instructions would then describe the checked-out tree, not the installed binary, and the guarantee the change exists to provide would be gone. It also splits the source of truth: an agent could read the repository copy while running a different version.

**Embed the corpus with `include_str!` from outside the crate.** It compiles and installs, but it breaks `cargo package` in a way that only surfaces when someone packages the crate. The failure mode is remote from the cause.

**Give the MCP server a `uservoice_skills` tool.** The corpus describes the CLI and the tools together; splitting it across two surfaces duplicates the content and halves the guarantee. The server and the client ship together, so pointing an MCP client at the CLI costs one line of server instructions.

**Let agents install the corpus into `~/.agents/skills`, as `gcx agent skills install` does.** Worth adding later. It is an installation convenience, not a correctness requirement, and it introduces a second copy that can go stale.

**Lint the prose.** Em dashes and bold-as-decoration are banned by the `/humanizer` style, but the repository uses 28 em dashes in its own source and 4 in `CONTEXT.md`. A lint that bans them in the corpus would contradict the house style and produce more noise than signal. Style is a review concern; drift is a build concern.

## Consequences

`build.rs` becomes the first build script in the workspace. It discovers skill files and writes a generated table; it fails the build when the corpus directory is missing, so a stripped checkout cannot silently produce a binary without instructions.

`crates/rusty_user_voice/src/skills.rs` owns the catalog and the lookup. `App` gains no skill method: skills describe the surface of the CLI, not the domain of UserVoice.

`CommandError` moves out of `login.rs` into `cli_error.rs`, because the skills command needs the same classified failure shape. The reason tokens become named constants with a `COMMAND_REASONS` slice, so the contract test reads the same source of truth the code uses. The public shape of `CommandError` does not change.

`OutputMode::resolve` becomes the single owner of the output-mode precedence, so `login`, `logout`, and `skills` share one rule instead of three copies. `skills` has no `--json` of its own, because the global `--output` already answers that question.

`tests/skills_contract.rs` becomes the enforcement point for corpus and code agreeing. A pull request that changes a flag, a tool, an error token, or an envelope status without updating the corpus fails CI.

`CONTEXT.md` gains the terms Skill, Skill Reference, and Skill Catalog, and its statement about where an intent is defined changes, because the corpus is now in this repository.

`docs/agents/skills.md` states the maintenance contract for anyone changing the CLI.
