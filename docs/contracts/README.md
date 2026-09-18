# Contracts

**Responsibility:** specify observable browser behavior, identities,
permissions, lifecycle, failures, and compatibility before implementation.

**Boundary:** contracts describe guarantees, not Chromium details, UI policy, plugin
implementation, or a frozen API. Changes must account for every consumer and
the corresponding contract and upgrade evidence.

**Status:** the [engine capability matrix](engine-capabilities.md) records draft
acceptance requirements; all runtime capabilities remain unverified. No product
behavior or API is implemented. The [engine v0 proposal](../../engine/chromium/DESIGN.md)
specifies candidate API semantics, not a frozen SDK. See the
[module map](../../MODULES.md).
