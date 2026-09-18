# Test fixtures

**Responsibility:** hold controlled websites, disposable profiles, malformed
packages, and known migration inputs needed by browser, security, and upgrade
tests.

**Boundary:** fixtures must contain synthetic data only—never personal profiles,
cookies, credentials, history, or copied private artifacts.

**Status:** this shared fixture directory remains a scaffold. Current synthetic
fixtures are owned by `apps/browser` and `apps/embedder_test` because they test
those applications directly. See the [module map](../../MODULES.md).
