---
name: feedback-loop
description: Close the feedback loop with supporters. Use when a fix ships, when a support ticket maps to an Idea, or when a supporter asks for an update.
---

# Close the feedback loop

Closing the loop means a supporter who asked for something hears what
happened. Run `uservoice-cli skills get core` first for the read loop, the write
loop, and the error vocabulary.

## 1. Find the Idea

Search by topic, then confirm with the full context.

```bash
uservoice-cli ideas search --query "csv export" --per-page 20
```

When the Idea ID is already known, skip the search:

```bash
uservoice-cli ideas get 101 --detail-level full --include-supporters
```

One Idea can carry feedback from several sources. Read the Feedback Records
to see what supporters actually said, when they said it, and which channel it
came from.

## 2. Record what arrived outside UserVoice

Feedback collected in a call, an email, or a ticket is not in UserVoice yet.
Record it with its provenance so the history stays complete.

```bash
uservoice-cli --output json feedback record \
  --body "Requested a downloadable roadmap for the exec review" \
  --channel api \
  --idea-id 101 \
  --source-url https://crm.example.invalid/cases/42 \
  --source-guid case-42 \
  --idempotency-key feedback-case-42
```

## 3. Publish the update

The Status Update reaches the Supporters. It is public, so it states what
shipped, not what might ship.

```bash
uservoice-cli --output json ideas publish 101 \
  --mode preview \
  --status-id 5 \
  --message "CSV export shipped in 2.4. Thank you for the requests that got it there." \
  --notify-supporters
```

Read the preview as a supporter would. Then apply it with the Change ID, the
confirmation, and a fresh Idempotency Key.

```bash
uservoice-cli --output json ideas publish 101 \
  --mode apply \
  --status-id 5 \
  --message "CSV export shipped in 2.4. Thank you for the requests that got it there." \
  --notify-supporters \
  --change-id "$CHANGE_ID" \
  --confirmation apply \
  --idempotency-key close-loop-101-2026-01-08
```

Apply repeats the preview payload exactly. A changed message between preview
and apply is a stale preview, and the ledger rejects it.

## 4. Leave a Note for the team

The internal Note records the decision and never reaches a supporter.

```bash
uservoice-cli --output json ideas update 101 \
  --mode preview \
  --internal-note "Loop closed 2026-01-08: CSV export shipped in 2.4, requester notified."
```

Apply it the same way as any other update.

## Before publishing

- The message names what shipped. "We are looking at this" is not a closure.
- The Status ID is the one the Idea should now carry. Check the catalog.
- `notify-supporters` is a decision. Turning it on sends mail to every
  Supporter of the Idea.
- The Idea context was read after the change, not before it.
