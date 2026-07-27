# Mutation Testing: Find Weak Tests

**Status:** ready to implement.
**Scope:** single commit — one-shot mutation run on business logic, analyze results, strengthen weak tests.
**Tool:** cargo-mutants.
**Duration:** ~1-2 hours on M5 Pro 64GB.

## Decisions

- **D1: Find weak tests** — primary goal. Not coverage improvement for its own sake.
- **M2: Business logic only** — mutate `app/read.rs`, `app/collect.rs`, `app/mutate.rs`, `app/feedback.rs`, `contracts/params.rs`. Skip I/O entry points.
- **S2: Find specific holes, not just ratio** — actionable per-mutation analysis in `.dev/MUTATION-RESULTS.md`.
- **R3: JSON output + our processing** — parse missed mutations, group by module, write results.
- **W3: One commit: run + analyze + fix tests** — don't stop at "we ran it".
- **C1: `--test-threads=1`** — config tests are flaky in parallel, 12 hours is enough.
- **F3: Case-by-case fixes** — critical mutations → new test; trivial → accepted in results file.
- **I1: Manual run only, no CI integration** — cargo-mutants is too slow for CI.

## Implementation Steps

### Step 1: Install cargo-mutants
```bash
cargo install cargo-mutants
```

### Step 2: Run mutation testing on business logic
```bash
cargo mutants --workspace --test-threads=1 \
  --exclude "main.rs" \
  --exclude "bootstrap.rs" \
  --exclude "cli_runner.rs" \
  --exclude "mcp.rs" \
  --exclude "bin/*" \
  --output json \
  --log-file /tmp/mutants.log
```

Scope: `app/read.rs`, `app/collect.rs`, `app/mutate.rs`, `app/feedback.rs`, `contracts/params.rs`.

### Step 3: Analyze results
Parse JSON output:
- Group missed mutations by module
- For each missed mutation: what was mutated, which test should catch it, is it critical
- Write analysis to `.dev/MUTATION-RESULTS.md`

### Step 4: Fix weak tests
For each critical missed mutation:
- Write a failing test that catches it
- Verify the mutation is now caught
- If mutation is trivial (error messages, formatting) → mark as accepted

### Step 5: Run full quality gate
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --test-threads=1
```

### Step 6: Commit
Single commit with:
- Mutation results in `.dev/MUTATION-RESULTS.md`
- Any new/updated tests
- No code changes to production code (unless a mutation revealed a real bug)

## Completion criterion
- cargo-mutants run completes
- `.dev/MUTATION-RESULTS.md` exists with per-module analysis
- Any critical missed mutations have corresponding new tests
- Quality gate green
