# Workspace preset

**Responsibility:** provide the workspace-oriented demo definition and grow
into the primary reference browser using public UI, operation, task, and plugin
contracts.

**Boundary:** the preset owns presentation and workspace policy only. It gets no
private access to core, Chromium, platform internals, credentials, or recovery state.

**Status:** [`definition.json`](definition.json) is implemented and exercised
through the same parser and native browser composition as the classic
definition. It is not yet a standalone distributable preset package. See the
[module map](../../MODULES.md).
