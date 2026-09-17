# Account-routing reference plugin

**Responsibility:** demonstrate a replaceable policy that selects among profile
contexts already granted by the host, including explicit-choice and
user-configured rule-based behavior.

**Boundary:** the plugin cannot enumerate unauthorized profiles, grant access,
create pages directly, or silently substitute a profile after failure. The host
must validate every result.

**Status:** scaffold only; no service contract or implementation exists. See
the [module map](../../../MODULES.md).
