# Tools

**Responsibility:** contain repository utilities now and, eventually, local
capability inspection, package validation, isolated preview, diagnostics,
activation, and recovery commands.

**Boundary:** Python is used only for repository tooling; it is not a selected
product language. Tools expose and inspect product behavior but do not own
upgrade policy, durable migration state, or the trusted recovery path.

**Status:** [`render_assets.py`](render_assets.py) renders the checked-in concept
artwork. The obsolete directory/link checker was removed during the implemented
layout migration. No general product build/preview toolchain exists yet. Do not
add scaffold or checkout-only validators and tests as a substitute for
implementing browser behavior.

See [source-build preparation](../embedder/chromium/BUILDING.md) for actual build
prerequisites and [the module map](../MODULES.md) for ownership.
