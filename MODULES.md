# Module map

This map records the implemented source ownership. The reusable Rust host API
lives under `embedder/public/rust/`, its canonical C ABI under
`embedder/public/c/`, private Chromium integration under `embedder/chromium/`,
and executable compositions under `apps/`. Source and build work do not, by
themselves, establish complete runtime acceptance.

The [repository layout](docs/repository-layout.md) also records planned core,
service, plugin, and platform boundaries. Directories are added only when a
real feature needs them.

## Modules and allowed dependencies

| Module | Owns | May depend on |
| --- | --- | --- |
| [`docs/contracts/`](docs/contracts/) | Observable behavior, permissions, lifecycle, and compatibility promises | No product module |
| [`docs/decisions/`](docs/decisions/) | Recorded architecture decisions and rejected alternatives | Evidence from any module |
| [`apps/browser/`](apps/browser/) | Native customization-demo composition, trusted recovery controls, and AppKit host | Embedder and UI public APIs; selected definitions |
| [`apps/embedder_test/`](apps/embedder_test/) | Independent manual API host, deterministic examples, diagnostics, and app packaging | Embedder public API |
| [`embedder/public/rust/`](embedder/public/rust/) | Trusted-host Rust API and framework linkage | Canonical C ABI behavior and packaged native artifact |
| [`embedder/public/c/`](embedder/public/c/) | Canonical narrow Rust/native ABI | C-compatible types only |
| [`embedder/chromium/`](embedder/chromium/) | Chromium Content implementation, GN integration, helpers, resources, source pin, and native build instructions | Selected Chromium public interfaces/components; embedder C ABI |
| [`core/`](core/) | Trusted identities, profile/data integrity, operations, tasks, authorization, and recovery coordination | Contracts only |
| [`platform/macos/`](platform/macos/), [`platform/linux/`](platform/linux/), [`platform/windows/`](platform/windows/) | OS integration, native hosting, packaging, and update integration | Core and engine public interfaces, contracts |
| [`ui/definition/`](ui/definition/) | Bounded JSON definition parsing, validation, state transitions, and address-open policy | No embedder or native implementation |
| [`ui/`](ui/) | Future broader UI bindings and native rendering boundary | Core and platform public interfaces, contracts |
| [`plugins/runtime/`](plugins/runtime/) | Plugin grants, isolation, lifecycle, and service registration | Core public interfaces and contracts |
| [`plugins/reference/account-routing/`](plugins/reference/account-routing/) | Replaceable reference account-routing policy | Plugin service contracts only |
| [`presets/workspace/`](presets/workspace/), [`presets/classic/`](presets/classic/) | Distinct implemented demo definitions and future complete reference packages | UI and declared public service contracts |
| [`tools/`](tools/) | Local inspection, validation, preview, diagnostics, and packaging commands | Published contracts and package formats |
| [`tests/`](tests/) | Contract, browser, security, upgrade, and fixture evidence | Any public test surface required by a scenario |

Dependencies point toward contracts and trusted mechanisms. Presets and plugins
must not access Chromium, platform internals, or core implementation details. Core
must not depend on a preset, UI policy, reference plugin, or developer tool.
Platform adapters must not choose browser-product policy.

## Planned browser outputs

- **Workspace preset:** the planned primary ready-to-use browser experience.
- **Classic preset:** a structurally different tab-oriented reference browser.

Both outputs must use the same public contracts available to personal packages.
Their JSON definitions run through the same `pliant-ui-definition` API and
native browser composition. Complete distributable presets and the broader
browser platform remain planned. Real native tests, not source layout, govern
acceptance.

## Upgrades and recovery

Upgrade and recovery are product responsibilities, not features hidden in
developer tools:

| Concern | Owner |
| --- | --- |
| Core data integrity, migration coordination, activation state, and the trusted recovery state machine | `core/` |
| Chromium source/dependency baseline, upstream adaptation, engine-data compatibility, backup boundaries, and migration evidence | [embedder/chromium/](embedder/chromium/) |
| Signed update and packaging integration for each OS | The corresponding `platform/` adapter |
| UI package compatibility and deterministic UI-package migrations | `ui/`, coordinated through core recovery |
| Plugin compatibility, grant revalidation, disablement, and quarantine | `plugins/runtime/`, coordinated through core recovery |
| Old packages, interrupted upgrades, and recovery evidence | `tests/upgrade/` |

The platform must host a trusted recovery entry point that presets and plugins
cannot replace. Tools may initiate or inspect upgrades and recovery, but they do
not own policy, durable state, or the recovery path.
