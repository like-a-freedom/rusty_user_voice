## Agent skills

### Issue tracker

Issues and specs for this repo are tracked as local markdown files under `.scratch/`. See `docs/agents/issue-tracker.md`.

### Triage labels

This repo uses the default mattpocock/skills triage labels. See `docs/agents/triage-labels.md`.

### Domain docs

This repo uses a single-context domain documentation layout. See `docs/agents/domain.md`.

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
