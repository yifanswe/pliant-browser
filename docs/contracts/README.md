# Contracts

**Responsibility:** specify observable browser behavior, identities,
permissions, lifecycle, failures, and compatibility before implementation.

**Boundary:** contracts describe guarantees, not Chromium details, UI policy, plugin
implementation, or a frozen API. Changes must account for every consumer and
the corresponding contract and upgrade evidence.

**Status:** the [engine capability matrix](engine-capabilities.md) records draft
acceptance requirements; all complete scenarios remain unverified. The narrow
trusted-host API and customization-demo behaviors are implemented and tested in
their owning modules, but they do not satisfy the broader platform contracts.
The [embedder v0 proposal](../../embedder/DESIGN.md) specifies candidate API
semantics beyond the MVP, not a frozen SDK. See the [module map](../../MODULES.md).
