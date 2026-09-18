# Classic preset

**Responsibility:** provide a traditional toolbar-and-tabs definition and grow
into a complete reference browser that proves the platform does not hard-code
the workspace experience.

**Boundary:** the preset uses the same public capabilities as the workspace
preset and personal packages, with no private core, Chromium, platform, credential,
or recovery access.

**Status:** [`definition.json`](definition.json) is implemented and exercised
through the same parser and native browser composition as the workspace
definition. It is not yet a standalone distributable preset package. See the
[module map](../../MODULES.md).
