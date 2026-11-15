---
name: triage
description: Triage new and updated Ideas. Use when a PM needs to review incoming feedback and decide what happens to each Idea.
---

# Triage Ideas

Triaging means deciding what happens to a batch of Ideas: which get a label,
which get a Status, which get a Note, and which need nothing at all. Run
`uservoice-cli skills get core` first for the read loop, the write loop, and
the error vocabulary.

## 1. Load the taxonomy

```bash
uservoice-cli workspace --include-catalog
```

Note the Forums, Categories, Statuses, and Labels. Every ID used later comes
from here. A label ID guessed from a previous session is a guess.

## 2. Choose the window

Triage a bounded window, not everything. Ideas updated in the last week, in
one Forum, is a batch a person can hold in their head.

```bash
uservoice-cli ideas search --forum-ids 11 --updated-after 2026-01-01T00:00:00Z --per-page 100
```

Use the taxonomy filter rather than a text query. Text search alone returns
noisy matches and hides the quiet Ideas that need attention.

For a large window, use `ideas collect` with a checkpoint so the run can
resume:

```bash
uservoice-cli --output ndjson ideas collect \
  --updated-after 2026-01-01T00:00:00Z \
  --forum-ids 11 \
  --checkpoint-name weekly-triage
```

Run `uservoice-cli skills get core --ref pagination.md` for cursors, `--all`,
and checkpoints.

## 3. Read each candidate

A search result is a summary. Load the full context before deciding.

```bash
uservoice-cli ideas get 101 --detail-level full --include-supporters
```

Read the comments, the Notes, the Feedback Records, and the activity. The
supporter count and the activity history say how much attention an Idea has
already had.

## 4. Record what you heard

When the triage session produced feedback that is not yet in UserVoice, record
it as a Feedback Record with its provenance.

```bash
uservoice-cli --output json feedback record \
  --body "Three teams asked for the same export" \
  --channel api \
  --idea-id 101 \
  --source-guid triage-2026-01-08 \
  --idempotency-key triage-101-2026-01-08
```

Use the channel the feedback really came from. The source URL and the source
GUID make the record traceable back to where it was collected.

## 5. Apply the decision

Preview first. The preview lists every field that changes, which is the moment
to notice that a label is wrong.

```bash
uservoice-cli --output json ideas update 101 \
  --mode preview \
  --add-label-ids 14 \
  --internal-note "Triaged 2026-01-08: three teams requested it"
```

Then apply with the Change ID, the confirmation, and a fresh Idempotency Key.

```bash
uservoice-cli --output json ideas update 101 \
  --mode apply \
  --add-label-ids 14 \
  --internal-note "Triaged 2026-01-08: three teams requested it" \
  --change-id "$CHANGE_ID" \
  --confirmation apply \
  --idempotency-key triage-update-101-2026-01-08
```

The Note explains the decision to the next person who reads the Idea. Write
the reason, not the restatement of the action.

## 6. Publish when the decision is public

A Status Update goes to the Supporters. Publish one only when the decision
is something supporters should read.

```bash
uservoice-cli --output json ideas publish 101 \
  --mode preview \
  --status-id 5 \
  --message "We are looking at this for the next release."
```

Preview, then apply with the Change ID and an Idempotency Key.

## What to record in a Note

The reason, the evidence, and the date. A Note that says "triaged" teaches the
next reader nothing. A Note that says "three enterprise accounts requested CSV
export in January; scoped for the next release" tells them why the Idea moved.
