## Agent skills

### Issue tracker

Issues and specs for this repo are tracked as local markdown files under `.scratch/`. See `docs/agents/issue-tracker.md`.

### Triage labels

This repo uses the default mattpocock/skills triage labels. See `docs/agents/triage-labels.md`.

### Domain docs

This repo uses a single-context domain documentation layout. See `docs/agents/domain.md`.

### Built-in skills

The agent-facing instructions are compiled into the binary. The source of truth is `crates/rusty_user_voice/skills/`; `uservoice-cli skills get core` serves it.

**When you change the tool, change the skills in the same commit.** A change to a CLI flag, an MCP tool, the envelope shape, a reason token, an exit code, or a workflow rule must update the matching skill file. `crates/rusty_user_voice/tests/skills_contract.rs` fails the build when the corpus and the code drift apart, so an unmaintained skill breaks CI rather than reaching an agent.

See `docs/agents/skills.md` for the authoring rules and the maintenance contract.

### Code Navigation Rules

ALWAYS use octocode MCP tools before reading files directly:
- Use `semantic_search` to find relevant code by meaning
- Use `view_signatures` to understand file structure
- Use `graphrag` to explore dependencies between files
- Use `structural_search` for AST-level pattern search (replaces grep/rg)

NEVER:
- Run grep, rg, find to locate code — use semantic_search instead
- Read entire files to understand structure — use view_signatures instead
- Guess file locations — use graphrag overview first

WORKFLOW for any task:
1. graphrag overview → understand project structure
2. semantic_search → find relevant files
3. view_signatures → inspect structure of found files
4. Read only specific sections if needed
