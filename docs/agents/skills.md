# Built-in skills

The agent-facing instructions ship inside the binary. `uservoice-cli skills get core` prints the guide for the installed version, so the text an agent reads can never describe a tool that does not exist.

## Layout

```text
crates/rusty_user_voice/skills/
  core/SKILL.md                      # the always-loaded layer
  core/references/cli.md             # one reference per topic
  core/references/mcp-tools.md
  core/references/pagination.md
  core/references/errors.md
  core/references/envelope.md
  triage/SKILL.md                    # a specialized workflow
  feedback-loop/SKILL.md
```

`build.rs` discovers the files and generates a table of path and content. `src/skills.rs` turns that table into a catalog; `src/skills_cli.rs` serves it.

A directory is a skill when it holds a `SKILL.md`. Its references sit in `<skill>/references/`. The build script rejects anything else, so a stray file fails the build instead of crashing the installed binary.

## The maintenance contract

**A change to the tool changes the skills in the same commit.**

| You changed | Update |
|---|---|
| A CLI flag, or its default | `core/references/cli.md` |
| An MCP tool, or one of its parameters | `core/references/mcp-tools.md` |
| A reason token, an exit code, an envelope status | `core/references/errors.md` |
| The envelope fields | `core/references/envelope.md` |
| Cursor, checkpoint, or streaming behavior | `core/references/pagination.md` |
| The read loop, the write loop, or a safety rule | `core/SKILL.md` |
| A triage or feedback-loop step | `triage/SKILL.md`, `feedback-loop/SKILL.md` |

`crates/rusty_user_voice/tests/skills_contract.rs` checks each row of that table. A flag that is not documented, a tool that is not documented, a reason token that is not documented, an orphaned reference, or a file over its word budget fails the build.

The test reads the flags out of the clap tree, the tool names out of `ToolName::ALL`, the envelope fields out of the schema, and the command paths out of the clap tree, so the list of what must be documented comes from the code, never from a hand-kept copy.

It also runs every `uservoice-cli` command found in the corpus through the argument parser. A command the CLI rejects is a broken product, not a typo in the documentation.

## Progressive disclosure

The corpus is layered, so an agent loads what the task needs and nothing more.

| Layer | Content | When it is read |
|---|---|---|
| Discovery | `skills list` gives names, descriptions, and reference counts | Choosing a skill |
| Always loaded | `core` carries the read loop, the write loop, safety rules, the error vocabulary, and a map of what to load next | Every task |
| Specialized | `triage` and `feedback-loop` | The matching task only |
| Detail | The five references | One topic at a time |

Two rules keep the layering honest:

- A skill links every reference it owns. An unlinked reference is unreachable, and the test fails.
- `core` links the specialized skills, so one read of `core` tells an agent what else exists.

## Authoring rules

**Write for an agent that will act.** Every section ends in something the reader can run: a command, a decision, or a recovery step. A paragraph that only restates the tool is dead weight.

**Prefer the command over the description.** `uservoice-cli ideas get 101 --detail-level full` teaches more in one line than a paragraph about what a full detail level includes.

**Keep the vocabulary of `CONTEXT.md`.** Idea, Preview, Apply, Change ID, Idempotency Key, Feedback Record, Forum, Category, Label, Status, Supporter. The glossary is the source of truth for what a word means.

**Stay inside the word budget.** The test enforces it:

| File | Budget |
|---|---|
| `core/SKILL.md` | 1800 words |
| a reference | 700 to 1200 words |
| a specialized skill | 900 words |
| the whole corpus | 6000 words |

A file over its budget is a sign that it mixes two topics. Split it into a second reference.

**Progressive disclosure inside a file too.** Lead with the decision, put the detail behind it, and use a table when the content is a list of facts.

**Every cross-reference is a command.** A reference that says "see `cli.md`" sends an agent to a file it cannot open. Say `uservoice-cli skills get core --ref cli.md` instead. The "load more" map at the end of `core` is the entry point, and every row in it is a runnable command.

**Plain prose.** Short sentences, active voice, no filler. The repository style tolerates an em dash in a code comment; the corpus is read by a model under a context budget, so it stays plain.

## Adding a skill

1. Create `crates/rusty_user_voice/skills/<name>/SKILL.md` with frontmatter:

   ```yaml
   ---
   name: <name>
   description: One line, saying when to reach for this skill.
   ---
   ```

2. Put anything longer in `crates/rusty_user_voice/skills/<name>/references/`.
3. Link every reference from the skill's `SKILL.md`.
4. Link the new skill from `core/SKILL.md`, so an agent that read only `core` learns it exists.
5. Run `cargo test -p rusty_user_voice --test skills_contract`. The word budgets and the link checks apply to the new files as well.

The build script discovers the new files on its own. No Rust code changes.

## Reading the corpus

```bash
uservoice-cli skills list
uservoice-cli skills get core
uservoice-cli skills get core --ref cli.md
uservoice-cli skills get core --full
uservoice-cli skills get --all
```

The command needs no credentials, so it works before `uservoice-cli login`.
