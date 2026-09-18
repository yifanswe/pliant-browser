# Contract tests

**Responsibility:** hold implementation-independent tests for observable
behavior shared by engines, platforms, presets, and authorized callers.

**Boundary:** these tests must not encode private Chromium details or one preset's UI
policy.

**Status:** this cross-module suite remains a scaffold. Implemented behavioral
tests currently live with `ui/definition`, `apps/browser`, and
`apps/embedder_test`; obsolete directory-only tests were removed. See the
[module map](../../MODULES.md).
