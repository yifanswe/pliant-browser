# Browser integration tests

**Responsibility:** exercise the real Chromium embedder, native windows, profiles, input, focus,
permissions, lifecycle, and supported desktop-platform scenarios.

**Boundary:** passing model or scaffold checks cannot substitute for these
tests. Platform claims require evidence from the named platform.

**Status:** this cross-module suite remains a scaffold. Implemented module-local
behavior tests live in [`apps/browser/tests/`](../../apps/browser/tests/) and
the independent manual/native API host lives in
[`apps/embedder_test/`](../../apps/embedder_test/). Those narrower tests do not
qualify the complete scenarios in the
[capability matrix](../../docs/contracts/engine-capabilities.md).
