# Tools

**Responsibility:** contain dependency-free repository checks now and,
eventually, local capability inspection, package validation, isolated preview,
diagnostics, activation, and recovery commands.

**Boundary:** Python is used only for repository tooling; it is not a selected
product language. Tools expose and inspect product behavior but do not own
upgrade policy, durable migration state, or the trusted recovery path.

**Status:** [check_structure.py](check_structure.py) checks required paths and
local Markdown links. No product build/preview toolchain exists yet. Do not add
scaffold or checkout-only validators and tests as a substitute for implementing
the browser; new tests should protect meaningful product behavior or regressions.

See [source-build preparation](../engine/chromium/BUILDING.md) for actual build
prerequisites and [the module map](../MODULES.md) for ownership.
