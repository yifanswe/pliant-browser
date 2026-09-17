# CEF engine adapter

**Responsibility:** host CEF and translate navigation, page lifecycle, input,
rendering, storage, permissions, and failures into Pliant contracts.

**Boundary:** Chromium handles and profile formats stay private to this adapter.
It does not define browser UI, profile policy, tasks, plugin grants, or
platform-specific packaging. CEF is selected; no distribution or integration
is present.

**Status:** scaffold only. See the [module map](../../MODULES.md).
