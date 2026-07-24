# UserVoice

Ubiquitous language for the UserVoice adapter and application: terms used by agents, the CLI, and the MCP surface. Source-of-truth names for the domain are listed here; code, docs, and tools all defer to this list.

## Domain nouns

**Idea**:
A unit of feedback inside UserVoice — the canonical noun used everywhere outside the wire format. Every idea has a stable numeric ID returned by the API.
_Avoid_: Suggestion, ticket, request, item, thread

**Suggestion**:
The UserVoice wire-format noun for an Idea. Appears only inside the `uservoice_client` adapter and in JSON field names that mirror the upstream API.
_Avoid_: outside the adapter crate

**Tenant**:
A single UserVoice installation (subdomain + admin/API domain pair). The CLI and MCP server target exactly one tenant at a time, configured by `USERVOICE_BASE_URL`.
_Avoid_: workspace, organization, account, instance

**Workspace**:
The agent-facing concept "the tenant as the caller sees it" — composed of the principal, the forum/category/status/label taxonomy, and the rate-limit envelope that came with it.
_Avoid_: tenant (when describing the agent-visible bundle)

**Forum**:
A top-level container of ideas inside a tenant. Each idea belongs to exactly one forum.
_Avoid_: board, project, collection

**Category**:
A forum-scoped label that organises ideas within a forum.
_Avoid_: tag, label (within a forum)

**Label**:
A cross-forum tag applied to ideas. Distinct from categories.
_Avoid_: tag (outside UserVoice docs), category

**Status**:
The lifecycle state of an idea (open, closed, archived, etc.).
_Avoid_: state, stage

**Supporter**:
A user who has upvoted or subscribed to an idea. Distinct from the author.
_Avoid_: subscriber, voter, follower

**Status Update**:
A short message published from the agent to the idea's supporters. Distinct from internal notes, which only admins see.
_Avoid_: comment, reply, announcement

**Note**:
An internal-only message on an idea, visible to admins but never to supporters.
_Avoid_: comment, status update

**Feedback Record**:
An imported comment from a non-portal source (Support, Slack, email) attributed to a known idea. Carries provenance (channel + source URL/GUID).
_Avoid_: comment, ticket, message

## Workflow concepts

**Tool**:
An MCP-exposed callable. Tool names follow `{service}_{verb}_{noun}` with the `uservoice_` prefix (`uservoice_search_ideas`, `uservoice_get_idea_context`, etc.). Seven tools exist; new agents assume the seven are exhaustive.
_Avoid_: endpoint, command (when speaking about MCP)

**Intent**:
What an agent is trying to accomplish ("close the feedback loop", "triage ideas"). One intent may map to one tool or to a small ordered sequence of tools (preview → apply). Defined in the `uservoice-pm` skill, not in this repo.
_Avoid_: workflow, recipe

**Preview**:
A dry-run bundle for a mutation that lists every field change, the expected `updated_at` it was generated against, and an opaque `change_id`. Required gate for every write.
_Avoid_: dry run, draft, plan

**Change ID**:
Opaque token returned by a successful preview. Its lifetime is the preview TTL; submitting an apply with a stale change_id is a `Conflict`.
_Avoid_: request ID, transaction ID

**Apply**:
The execution step that consumes a `change_id` and performs the upstream write. Exactly once per change_id (the ledger rejects second applies).

**Idempotency Key**:
Caller-chosen string that prevents duplicate side-effects on retry. Same key + identical payload replays the recorded result; same key + different payload is a `Conflict`. Different keys are independent.
_Avoid_: request ID (without the same-key-replays-payload semantic)

**Checkpoint**:
A CLI-only resumption token (`uservoice collect-ideas --checkpoint-name …`). Captures the `continuation` cursor plus the time window so a long-failed collect run can resume. The MCP `uservoice_collect_ideas` tool does not store checkpoints.
_Avoid_: cursor (for the named, on-disk bundle), session

**Ledger**:
The atomic, on-disk directory holding preview JSONL records and idempotency JSON records. Lives under `USERVOICE_STATE_DIR`. Never contains message/note bodies, never contains the API token.
_Avoid_: state file, log

## Outcome language

**OK**:
The tool succeeded and every requested section / field is present.
_Avoid_: success (wire-format field; the in-code enum variant is `OutcomeStatus::Ok`)

**Partial**:
The tool succeeded for some sections but at least one optional section failed. The envelope carries `missing_sections` and a `next_actions` entry telling the caller how to resume.
_Avoid_: degraded, incomplete (these lose the resume semantic)

**No Results**:
A read returned an empty result set — not a failure. Distinct from `Conflict` and from `Conflict("change_id has expired")`.
_Avoid_: empty, none_found

**Conflict**:
The tool refused because prior state is incompatible with the request (stale preview, idempotency mismatch, pagination cap exceeded). The caller should re-read or re-preview, not retry as-is.
_Avoid_: error, failure, bad_request (for state-mismatch cases)

## Safety vocabulary

**Credential Boundary**:
The line separating the `USERVOICE_API_KEY` from everything the agent can see. The token enters the process only via the access-token wrapper, never appears in `Debug`, `tracing`, or MCP error payloads, and is never logged.
_Avoid_: api key (only inside config and process-local storage)

**Two-Phase Mutation**:
Every write follows preview → apply under an opaque change_id, with at-most-once apply enforced by the ledger. No mutation tool accepts effects in a single call.
_Avoid_: one-shot write, atomic write (without the preview gate)

**Provenance**:
For feedback imported from non-portal sources: the channel plus the source URL and/or source GUID. Provenance is part of the request to `uservoice_record_feedback` and a stored attribute on every recorded feedback record.
_Avoid_: metadata, context
