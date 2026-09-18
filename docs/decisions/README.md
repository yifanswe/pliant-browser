# Architecture decisions

**Responsibility:** record accepted decisions, evidence, trade-offs, and
rejected alternatives.

**Boundary:** this directory does not decide through placeholder code. The host
language, native UI framework, UI DSL, and plugin runtime remain undecided until
the planned feasibility work supplies evidence.

**Accepted direction:** [ADR 0001: own the Chromium embedding layer](0001-own-chromium-embedding.md)
replaces the earlier CEF backend plan. Acceptance selects the architecture, not
proof of a working or cross-platform implementation. Core/UI/runtime language
choices remain open. See the [module map](../../MODULES.md).
