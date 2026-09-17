# Tools

**Responsibility:** contain dependency-free repository checks now and,
eventually, local capability inspection, package validation, isolated preview,
diagnostics, activation, and recovery commands.

**Boundary:** Python is used only for repository tooling; it is not a selected
product language. Tools expose and inspect product behavior but do not own
upgrade policy, durable migration state, or the trusted recovery path.

**Status:** `check_structure.py` checks this scaffold and local Markdown links.
No browser development toolchain exists. See the [module map](../MODULES.md).
