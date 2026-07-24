# Close the feedback loop

A structured workflow for responding to customer feedback recorded in UserVoice.

## When to use

- After a feature ships that addresses recorded feedback
- When a support ticket references a UserVoice idea
- During quarterly reviews to update supporters on progress

## Steps

### 1. Find the related idea

```
uservoice_search_ideas
  query: <feature or topic>
  per_page: 20
```

Or if you have the idea ID directly:

```
uservoice_get_idea_context
  idea_id: <ID>
  detail_level: full
  include_supporters: true
```

### 2. Review the feedback history

Examine `feedback_records` in the idea context to understand:
- What customers said
- When they said it
- Which channel it came from (portal, email, Slack, etc.)
- Source URLs for traceability

### 3. Record new feedback if needed

If you received feedback outside UserVoice (e.g., in a call or email):

```
uservoice_record_feedback
  body: "<customer quote or summary>"
  channel: api
  idea_id: <idea ID>
  user_id: <if known>
  source_url: <CRM or ticket URL>
  source_guid: <unique identifier>
  idempotency_key: feedback-<source>-<date>
```

### 4. Publish a public status update

```
uservoice_publish_idea_update
  mode: preview
  idea_id: <ID>
  status_id: <shipped or completed status ID>
  message: "We've shipped this feature based on your feedback. Thank you for helping us improve!"
  notify_supporters: true
```

Review the preview, then apply:

```
uservoice_publish_idea_update
  mode: apply
  idea_id: <ID>
  status_id: <status ID>
  message: "We've shipped this feature based on your feedback. Thank you for helping us improve!"
  notify_supporters: true
  change_id: <from preview>
  confirmation: apply
  idempotency_key: close-loop-<idea_id>-<date>
```

### 5. Update internal metadata

```
uservoice_update_idea
  mode: preview
  idea_id: <ID>
  internal_note: "Feedback loop closed on <date>. Feature shipped in <release>."
```

Apply the preview to record the internal note.
