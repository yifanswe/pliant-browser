# Contract tests

**Responsibility:** hold implementation-independent tests for observable
behavior shared by engines, platforms, presets, and authorized callers.

**Boundary:** these tests must not encode private Chromium details or one preset's UI
policy.

**Status:** scaffold only; there are no browser contract tests. The existing
Python structure tests are repository tooling, not browser tests. See the
[module map](../../MODULES.md).
