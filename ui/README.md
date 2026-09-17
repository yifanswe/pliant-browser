# UI

**Responsibility:** eventually define and implement the declarative UI package
boundary, validation, state bindings, interactions, and native rendering.

**Boundary:** UI code uses public operations and observable state. It cannot
access CEF, protected profile data, credentials, or core implementation details.
The DSL syntax, host language, and native UI framework remain undecided.

**Status:** scaffold only; there is no compiler, renderer, or UI API. See the
[module map](../MODULES.md).
