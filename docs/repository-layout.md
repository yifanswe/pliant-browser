# Repository layout

**Status: implemented for the current browser, embedder, UI-definition, preset,
and tooling sources. Planned modules below are created only with real features.**

[Module map](../MODULES.md) · [Design philosophy](../DESIGN_PHILOSOPHY.md) · [Implementation plan](../IMPLEMENTATION_PLAN.md)

Separate the reusable embedder, trusted platform mechanisms, browser services, and executable applications. Treat agents as service participants, not as part of the web engine. Keep replaceable UI and behavior outside the trusted mechanisms.

This is the ownership layout, not a request to create every directory now. Add
implementation directories when their first real feature needs them. The
implemented migration covers `apps/`, `embedder/`, `ui/definition/`, preset
definitions, and repository tooling without selecting a generic core language,
UI toolkit, or plugin execution technology through directory naming.

**Current delivery scope:** finish the embedder MVP, then demonstrate independent, quick, safe browser customization without an agent service. `services/agent/` and its collaboration interfaces are deferred design placeholders in this document, not directories or APIs to implement now. Build only the modules required by that customization loop.

## What Chromium's `services` means

Chromium's top-level `//services` contains foundational services. Its guidance also identifies `//components/services` for reusable Content/embedder services and `//chrome/services` for Chrome-specific services. A separate process and a Mojo interface do not, by themselves, determine the directory.

A browser agent can span multiple modules. Its reusable service implementation may fit a service directory. Browser integration, permission enforcement, and UI belong to their respective owners. A service client does not need to become a service itself.

For Pliant, use our own `services/agent/` for the agent service contract and the built-in implementation. Shipping useful built-in assistance is a product goal; users can still replace or disable it. Do not add Pliant product code to the upstream Chromium tree. A user-supplied agent may run outside this repository and consume the same authorized interfaces.

## Target tree

```text
pliant-browser/
├── apps/
│   ├── browser/                  # Executable composition and trusted recovery entry
│   └── embedder_test/            # Independent manual and automated API test app
├── embedder/
│   ├── public/
│   │   ├── rust/                 # Trusted-host Rust API and crate
│   │   └── c/                    # Narrow native ABI shared with the Rust binding
│   └── chromium/                 # Content integration, helpers, resources, GN build
├── core/
│   ├── public/                   # Shared identities and capability/lifecycle types
│   ├── capabilities/             # Grants, revocation, and common enforcement
│   ├── hosting/                  # Participant admission, bindings, process lifecycle
│   ├── operations/               # Correlation, ownership, cancellation, audit state
│   └── recovery/                 # Activation, compatibility, recovery coordination
├── services/
│   ├── page/                     # Authorized page operations over the embedder
│   ├── profile/                  # Profile identity, data allocation and isolation
│   ├── agent/                    # Built-in assistance and coding-agent collaboration
│   ├── session/                  # Session organization and restoration service
│   ├── download/                 # Download lifecycle and destination-policy boundary
│   └── credential/               # Protected credential-provider boundary
├── platform/
│   ├── public/                   # Host facilities required by portable modules
│   ├── macos/
│   ├── linux/
│   └── windows/
├── ui/                           # DSL, bindings and native UI rendering
├── plugins/
│   ├── runtime/                  # Package loading, execution isolation and limits
│   └── reference/account-routing/ # Replaceable policy provider
├── presets/
│   ├── workspace/                # Complete reference UI and service selection
│   └── classic/                  # Contrasting reference UI and service selection
├── tests/                        # Cross-module E2E, security, upgrade tests, fixtures
├── tools/                        # Developer commands and packaging orchestration
├── docs/                         # Decisions and behavioral explanations
└── assets/                       # Documentation and concept artwork
```

`embedder/` replaced the previous `engine/chromium/` grouping because Pliant
owns an embedder, not a selection of ready-made engine adapters. Chromium-
specific code stays private. This naming does not require another backend, a
universal engine framework, or a stable public native ABI in the first MVP.

The Rust-facing API and the native C ABI are different boundaries. The C ABI is an implementation seam, not the UI/plugin/agent API. Its compatibility must match the packaged native artifact.

## Responsibilities that must stay distinct

| Boundary | Responsibility |
| --- | --- |
| `apps/browser/` versus `presets/` | The executable composes services and native hosting. A preset supplies the replaceable interface and behavior selection. Recovery stays outside the preset. |
| `apps/embedder_test/` versus `embedder/` | The test app calls the same Rust API as another trusted host. It provides minimal test controls and visible content, not a second browser product. |
| `embedder/` versus `services/page/` | The embedder owns Content objects, navigation, surfaces, and native resource lifetimes. The page service binds these mechanisms to authorized Pliant identities and operations. |
| `services/profile/` versus `embedder/` | The profile service selects and owns profile allocations. Chromium implements cookies and site storage inside the selected context. Neither invents a second cookie store. |
| `core/` versus `services/` | Core owns common trust and lifecycle mechanisms. Services own their domain state and behavior. Core must not accumulate every browser feature. |
| `core/hosting/` versus `plugins/runtime/` | Hosting owns shared admission and service binding. The plugin runtime implements package execution and reports lifecycle through hosting; it does not create a second authority or registry. |
| `services/agent/` versus `core/hosting/` | Agent runtime code chooses and performs work. Hosting controls whether the process can participate and which interfaces it receives. The agent cannot authorize itself. |
| `platform/` versus embedder native glue | Platform owns OS windows, containers, and system facilities. Embedder glue owns Chromium content-view attachment and engine-specific input integration. |

Services are logical ownership boundaries, not a mandate to launch one process per directory. Initially, trusted page/profile services can share the browser host process. The native agent has a separate process boundary by design. Its sandbox and permitted model/network access still need a concrete design.

The service implementation and its process entry point can remain in one service directory. Do not add a separate generic agent-host subsystem merely to launch the first agent. Reuse Chromium/Mojo process facilities where suitable, behind the existing host boundary.

## Two responsibilities of the built-in agent

The built-in agent serves everyday browsing and interactive browser customization. It is not only a process that accepts user tasks.

For everyday browsing, it consumes authorized content and behavior signals and supplies recommendations, predictions, or other assistance. Browser services can request that assistance or receive scoped suggestions. Keep context handling, assistance policies, and model orchestration inside the replaceable agent implementation. Do not make every page operation depend on synchronous inference.

For customization, it helps the user clarify a goal and exchanges questions, approved context, and preview feedback with the user's coding agent. The coding agent edits through `tools/` and the customization contracts. The built-in agent does not need unrestricted source-writing authority to support this dialogue. External coding agents may need a versioned adapter to the native contract; requiring them to embed Mojo directly is not a settled requirement.

These can begin as modules inside `services/agent/impl/`, not separate services or processes. Its `public/` contracts cover assistance and collaboration as well as lifecycle control. Introduce concrete interfaces only with a real scenario.

Execution remains with the relevant owner: `services/page/` and the embedder perform eligible speculative loading; the UI presents suggestions; the plugin/UI toolchain validates changes; core coordinates activation and recovery. The agent proposes and collaborates without bypassing those boundaries. These responsibilities belong to later platform work, not the active embedder MVP.

## Service-local contracts

Use this shape when a service is implemented:

```text
services/<name>/
├── public/
│   ├── mojom/                    # Versioned Pliant service messages
│   └── <language>/               # Client binding, only when needed
├── impl/                         # Reference service implementation
├── tests/                        # Unit and public-contract tests
└── README.md                     # Scope, permissions and supported usage
```

Keep service-owned schemas next to the service. Do not duplicate their definitions in a central API directory. `core/public/` contains only genuinely shared types. `docs/contracts/` explains observable behavior and links to executable definitions once those definitions exist.

Public bindings must not depend on a service's private implementation. Another implementation should be able to satisfy the contract. Services call each other's public interfaces, not each other's private state. Generated bindings and build outputs remain outside source directories.

Here, `public/` describes an import boundary, not unrestricted authority. The embedder API is for trusted hosts. A plugin or agent gets only the service interfaces and target scope that the platform grants.

Mojo is the intended process transport. This does not prove that every intended agent language has a usable binding in our selected build. Verify bindings, bootstrap, compatibility, and revocation with a small client before committing to a language-specific SDK. Pliant interfaces must not expose arbitrary Chromium-internal Mojo endpoints.

## Dependency direction

```text
apps/browser -> core + service implementations + platform + UI runtime
presets -> UI contracts + authorized service contracts
agent process -> authorized service contracts
plugin implementation -> declared service contracts
services/page -> embedder public API
services/profile -> embedder context API
services -> core public mechanisms + injected platform facilities
embedder/chromium -> Chromium Content and selected upstream components
```

The app is the composition root. It selects implementations and supplies host facilities. Core does not import the page service, UI, preset, or agent implementation. The embedder does not import the plugin runtime, task planner, tab/workspace model, or agent runtime.

Authorization need not turn core into a proxy for every message. Core grants a scoped binding; the receiving service enforces that scope and records operation outcomes. Revocation must invalidate existing access, not merely block future service discovery. The enforcement contract must remain valid when a service implementation is replaced.

Keep engine events and platform operations distinct. An embedder navigation event is not proof that an agent task succeeded or that a remote write was reversed.

## One native-agent request

The trusted host admits an agent process and establishes its identity over a controlled Mojo bootstrap. Hosting then grants a page-service binding restricted to an authorized profile and page set.

The agent calls the page service directly. The page service checks the binding and current target lifetime, then uses the embedder API. Chromium performs the navigation. The page service reports the actual outcome through its contract.

If access is revoked, subsequent unauthorized calls fail even on an existing pipe. Closing the agent process does not close the human's unrelated pages. Agent task planning and memory stay with the replaceable agent implementation; the embedder contains neither.

This example defines ownership, not finalized Mojo methods or an implemented agent runtime.

## Implemented migration

| Previous location | Implemented destination or treatment |
| --- | --- |
| `engine/chromium/mvp/src/lib.rs`, crate manifest and build script | Moved to `embedder/public/rust/`; native link consumers use the new Cargo path. |
| `engine/chromium/mvp/native/bridge.h` | Moved to `embedder/public/c/` as the single canonical ABI definition. |
| `engine/chromium/mvp/native/` implementation, GN files, helpers and resources | Moved to `embedder/chromium/`; test-app metadata is app-local. |
| `engine/chromium/mvp/examples/` | Moved to `apps/embedder_test/examples/` alongside the independent manual API app. |
| `engine/chromium/mvp/run_macos.sh` and `native/app.plist` | Replaced by safe explicit packaging under `apps/embedder_test/`; the manual test app retains its own plist. |
| `engine/chromium/{README,DESIGN,BUILDING}.md`, `upstream.json` | Embedder-wide material moved under `embedder/`; Chromium build/pin material moved under `embedder/chromium/`. |
| Broad profile/task/service duties described under `core/` | Keep shared trust mechanisms in core; move domain duties to their service when implemented. Agent planning does not become a core feature. |
| `plugins/runtime/` service registration | Consume `core/hosting/` admission/binding mechanisms rather than owning a parallel registry. |
| `plugins/reference/`, `presets/`, `ui/`, `platform/` | Retain their roles; adopt service-local contracts incrementally. |
| `scripts/render_assets.py` | Moved to `tools/render_assets.py`; existing artwork remains unchanged. |
| Existing structure-only checker/tests | Removed as obsolete without adding replacement bookkeeping tests. |

Native engine helpers belong to the embedder. They are not agent processes or application test fixtures. The current window code inside `bridge.mm` is provisional test hosting. Move native window/container ownership to the test app and platform adapter only when that boundary can be exercised without expanding the MVP into a general UI framework.

## Delivery order and acceptance

**Current:** the implemented embedder, test app, customization browser,
definitions, Cargo/GN inputs, source mapping instructions, and documentation use
the ownership paths above. The external Chromium checkout, dependencies, and
incremental output remain outside this repository.

**Migration acceptance:** independently build and run the test app through create/load A, load B, back, forward, close, and stale-ID rejection. Verify ordinary lifecycle handlers do not prevent close. Exercise the manual test controls through the same exported API. Record native rendering and helper startup separately from Rust type checking. A directory tree or a link check is not acceptance.

**Later:** implement the smallest page/profile service bindings, then a deterministic out-of-process agent client. Prove authorization, direct calls, revocation, and isolation before adding a model-driven runtime. Instantiate other service directories only as those features are implemented.

This layout changes ownership and organization, not the current MVP feature
budget. It does not imply dependency installation or publication.

## Source basis

Reviewed against the project's pinned Chromium revision `db8ceb709fe92f3bb010fb982d6300e54de6dc6a`:

- [Service development guidelines](https://chromium.googlesource.com/chromium/src/+/db8ceb709fe92f3bb010fb982d6300e54de6dc6a/services/README.md): service placement and public/private dependencies.
- [Components guidance](https://chromium.googlesource.com/chromium/src/+/db8ceb709fe92f3bb010fb982d6300e54de6dc6a/components/README.md): reuse and layering boundaries.
- [ServiceProcessHost](https://chromium.googlesource.com/chromium/src/+/db8ceb709fe92f3bb010fb982d6300e54de6dc6a/content/public/browser/service_process_host.h): process launch with sandbox selection and an initial Mojo interface. A launch API is not a complete Pliant service registry.
