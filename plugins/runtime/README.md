# Plugin runtime

**Responsibility:** eventually enforce plugin grants, isolation, lifecycle,
resource limits, cancellation, conflicts, and service registration.

**Boundary:** plugins use explicit service contracts and cannot patch core,
access platform or CEF internals, or gain authority from a returned value.
Chrome and Firefox extension compatibility is out of scope. The execution
technology and protocol remain undecided.

**Status:** scaffold only; no runtime or ABI exists. See the
[module map](../../MODULES.md).
