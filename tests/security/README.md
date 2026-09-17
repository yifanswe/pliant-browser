# Security tests

**Responsibility:** exercise permission denial, profile isolation, protected
data handling, plugin failure boundaries, resource limits, and adversarial
inputs.

**Boundary:** tests supply evidence; enforcement belongs to the runtime, core,
engine, and platform boundaries. Fixtures must use synthetic, disposable data.

**Status:** scaffold only; no security guarantee has been tested. See the
[module map](../../MODULES.md).
