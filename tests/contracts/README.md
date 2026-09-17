# Contract tests

**Responsibility:** hold implementation-independent tests for observable
behavior shared by engines, platforms, presets, and authorized callers.

**Boundary:** these tests must not encode private CEF details or one preset's UI
policy.

**Status:** scaffold only; there are no browser contract tests. The Python
structure test is repository tooling, not a browser test. See the
[module map](../../MODULES.md).
