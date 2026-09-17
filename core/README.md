# Core

**Responsibility:** provide trusted browser mechanisms and enforce invariants.

- **Profiles and data:** own stable profile identity, isolation, durable browser
  data boundaries, explicit deletion, migration coordination, and recovery.
  Engine-native storage remains behind its adapter.
- **Operations and tasks:** own stable page/window/task/operation identities,
  authorization, dispatch, cancellation, ownership, lifecycle, and auditable
  outcomes for human and agent callers.
- **Policy enforcement:** validate permissions and plugin results without
  embedding replaceable routing, credential, download, session, or UI policy.

**Boundary:** core exposes versioned contracts; it never depends on a preset,
reference plugin, UI implementation, developer tool, or CEF internal type.

**Status:** scaffold only; no data model or operation API is frozen. See the
[module map](../MODULES.md).
