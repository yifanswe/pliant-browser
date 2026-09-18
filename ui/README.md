# UI

**Responsibility:** define the declarative UI package boundary, validation,
state bindings, interactions, and native rendering.

**Boundary:** UI code uses public operations and observable state. It cannot
access Chromium, protected profile data, credentials, or core implementation details.
The broader DSL syntax, host language, and native UI framework remain open.

**Status:** [`definition/`](definition/) implements the bounded JSON definition,
validation, preview/apply/reject/reset state machine, and address-open policy
used by the native customization demo. The AppKit renderer remains app-local;
there is no general UI runtime or plugin-facing UI API yet. See the
[module map](../MODULES.md).
