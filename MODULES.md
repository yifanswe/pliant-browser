# Module map

This map defines the intended repository boundaries. Every product module is a
scaffold only; names, contracts, and implementation languages remain subject to
the evidence and decisions in [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md).

## Modules and allowed dependencies

| Module | Owns | May depend on |
| --- | --- | --- |
| [`docs/contracts/`](docs/contracts/) | Observable behavior, permissions, lifecycle, and compatibility promises | No product module |
| [`docs/decisions/`](docs/decisions/) | Recorded architecture decisions and rejected alternatives | Evidence from any module |
| [`core/`](core/) | Trusted identities, profile/data integrity, operations, tasks, authorization, and recovery coordination | Contracts only |
| [`engine/cef/`](engine/cef/) | CEF hosting and translation at the engine boundary | Core public interfaces and contracts |
| [`platform/macos/`](platform/macos/), [`platform/linux/`](platform/linux/), [`platform/windows/`](platform/windows/) | OS integration, native hosting, packaging, and update integration | Core and engine public interfaces, contracts |
| [`ui/`](ui/) | Future UI language, validation, evaluation, and native rendering boundary | Core and platform public interfaces, contracts |
| [`plugins/runtime/`](plugins/runtime/) | Plugin grants, isolation, lifecycle, and service registration | Core public interfaces and contracts |
| [`plugins/reference/account-routing/`](plugins/reference/account-routing/) | Replaceable reference account-routing policy | Plugin service contracts only |
| [`presets/workspace/`](presets/workspace/), [`presets/classic/`](presets/classic/) | Complete reference browser packages | UI, core public contracts, and declared plugin services |
| [`tools/`](tools/) | Local inspection, validation, preview, diagnostics, and packaging commands | Published contracts and package formats |
| [`tests/`](tests/) | Contract, browser, security, upgrade, and fixture evidence | Any public test surface required by a scenario |

Dependencies point toward contracts and trusted mechanisms. Presets and plugins
must not access CEF, platform internals, or core implementation details. Core
must not depend on a preset, UI policy, reference plugin, or developer tool.
Platform adapters must not choose browser-product policy.

## Planned browser outputs

- **Workspace preset:** the planned primary ready-to-use browser experience.
- **Classic preset:** a structurally different tab-oriented reference browser.

Both outputs must use the same public contracts available to personal packages.
Neither preset, browser binary, native shell, nor CEF integration exists yet.

## Upgrades and recovery

Upgrade and recovery are product responsibilities, not features hidden in
developer tools:

| Concern | Owner |
| --- | --- |
| Core data integrity, migration coordination, activation state, and the trusted recovery state machine | `core/` |
| CEF profile-format compatibility, engine-data backup boundaries, and engine migration evidence | `engine/cef/` |
| Signed update and packaging integration for each OS | The corresponding `platform/` adapter |
| UI package compatibility and deterministic UI-package migrations | `ui/`, coordinated through core recovery |
| Plugin compatibility, grant revalidation, disablement, and quarantine | `plugins/runtime/`, coordinated through core recovery |
| Old packages, interrupted upgrades, and recovery evidence | `tests/upgrade/` |

The platform must host a trusted recovery entry point that presets and plugins
cannot replace. Tools may initiate or inspect upgrades and recovery, but they do
not own policy, durable state, or the recovery path.
