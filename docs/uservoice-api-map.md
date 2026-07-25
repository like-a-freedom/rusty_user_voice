# UserVoice Admin API Map

This document maps the `uservoice_client` crate methods to their UserVoice Admin API HTTP endpoints.

## Read methods

| Client method | HTTP | Path | Notes |
|---|---|---|---|
| `current_user()` | GET | `/admin/users/current` | Auth probe; returns authenticated principal |
| `workspace_catalog()` | GET | `/admin/forums`, `/admin/categories`, `/admin/statuses`, `/admin/labels` | Drains all pages per collection |
| `search_suggestions(request)` | GET | `/admin/suggestions` | Query params: `q`, `forum`, `category`, `status`, `label`, `state`, `sort`, `updated_after`, `updated_before`, `per_page`, `cursor` |
| `suggestion(idea_id)` | GET | `/admin/suggestions/{id}` | `includes=labels,forums,status_updates,users,statuses,categories,attachments` |
| `comments(idea_id, per_page)` | GET | `/admin/comments` | `suggestion={id}`, follows pagination |
| `notes(idea_id, per_page)` | GET | `/admin/notes` | `suggestion={id}`, follows pagination |
| `supporters(idea_id, per_page)` | GET | `/admin/supporters` | `suggestion={id}`, follows pagination |
| `feedback_records(idea_id, per_page)` | GET | `/admin/feedback_records` | `suggestion={id}`, follows pagination |
| `status_updates(idea_id, per_page)` | GET | `/admin/status_updates` | `suggestion={id}`, follows pagination |
| `activity(idea_id, per_page)` | GET | `/admin/suggestion_activity_entries` | `suggestion={id}`, follows pagination |

## Write methods

| Client method | HTTP | Path | Notes |
|---|---|---|---|
| `update_suggestion(idea_id, request)` | PUT | `/admin/suggestions/{id}` | JSON body: title, body, links |
| `attach_label(idea_id, label_id)` | POST | `/admin/labels/attach` | JSON body: `{links: {label, suggestion}}` |
| `detach_label(idea_id, label_id)` | POST | `/admin/labels/detach` | JSON body: `{links: {label, suggestion}}` |
| `create_note(idea_id, body)` | POST | `/admin/notes` | JSON body: body, body_mime_type, links.suggestion |
| `create_status_update(request)` | POST | `/admin/status_updates` | JSON body: body, links, notify_supporters |
| `create_feedback_record(request)` | POST | `/admin/feedback_records` | JSON body: body, channel, links, source_url, source_guid |

## Retry policy

- GET methods: up to 3 attempts with exponential backoff + jitter, capped at 30s
- Section-level reads (idea context fan-out): retry once on 429, then drop to `partial`
- Write methods: no retry; result recorded in idempotency ledger

## Rate limiting

Every response may include `X-Rate-Limit-Limit`, `X-Rate-Limit-Remaining`, `X-Rate-Limit-Reset`.
429 responses include `Retry-After`. The client honors `Retry-After` for retry delays.
