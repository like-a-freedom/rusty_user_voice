---
name: uservoice-pm
description: Companion skill for AI product managers using the uservoice-mcp server and uservoice-cli tool.
---

# uservoice-pm

This skill moved into the binary. The instructions now ship with the tool, so
they always match the installed version.

```bash
uservoice-cli skills get core
```

`skills get core` prints the guide: authentication, the read loop, the
preview-apply write loop, safety rules, and the error vocabulary. It needs no
credentials, so it works before `uservoice-cli login`.

Load more only when the task needs it:

```bash
uservoice-cli skills list
uservoice-cli skills get core --ref cli.md
uservoice-cli skills get core --full
```

The former `references/` files are now the references of the `core` skill:
`cli.md`, `mcp-tools.md`, `pagination.md`, `errors.md`, and `envelope.md`.
