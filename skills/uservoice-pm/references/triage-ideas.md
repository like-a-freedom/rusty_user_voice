# Triage ideas

A structured workflow for triaging new or updated UserVoice ideas.

## Steps

### 1. Discover workspace taxonomy

```
uservoice_workspace_context
```

Note the available forums, categories, statuses, and labels. These IDs are used in subsequent searches and updates.

### 2. Search for recent ideas

Use `uservoice_search_ideas` only after you have `forum_ids` from `workspace_context`; text search alone returns noisy matches. For bulk extraction, prefer `uservoice_collect_ideas --forum-ids <id>` over text search.

```
uservoice_search_ideas
  forum_ids: [<forum ID from workspace_context>]
  updated_after: <start of triage window>
  updated_before: <end of triage window>
  status_ids: [<open status IDs>]
  per_page: 100
```

For bulk collection across many pages:

```
uservoice_collect_ideas
  updated_after: <start>
  forum_ids: [<forum ID>]
  detail_level: summary
  per_page: 20
```

Repeat with the returned `continuation` until absent.

### 3. Load full context for each candidate

```
uservoice_get_idea_context
  idea_id: <candidate ID>
  detail_level: full
  include_supporters: true
  per_page: 100
```

Review comments, internal notes, feedback records, and activity history.

### 4. Record feedback from the triage session

```
uservoice_record_feedback
  body: "<triage summary>"
  channel: api
  idea_id: <candidate ID>
  source_guid: triage-<date>
  idempotency_key: triage-<idea_id>-<date>
```

### 5. Apply triage decisions

```
uservoice_update_idea
  mode: preview
  idea_id: <candidate ID>
  add_label_ids: [<triage label IDs>]
  internal_note: "<triage decision rationale>"
```

Review the preview, then apply:

```
uservoice_update_idea
  mode: apply
  idea_id: <candidate ID>
  add_label_ids: [<triage label IDs>]
  internal_note: "<triage decision rationale>"
  change_id: <from preview>
  confirmation: apply
  idempotency_key: triage-update-<idea_id>-<date>
```

### 6. Optionally publish a public status update

```
uservoice_publish_idea_update
  mode: preview
  idea_id: <candidate ID>
  status_id: <new status ID>
  message: "<public message>"
  notify_supporters: false
```

Review and apply with `change_id`, `confirmation: apply`, and `idempotency_key`.
