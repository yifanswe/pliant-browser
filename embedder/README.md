# Chromium embedder

**Responsibility:** implement Pliant's engine-facing contracts directly over
Chromium's Content API and selected supporting components. Execute authorized,
explicitly targeted operations and translate engine state, requests, and
failures into Pliant terms.

**Boundary:** reuse Chromium's infrastructure, not the full Chromium browser
application. Chromium supplies web execution; core owns browser authority;
platform adapters supply native hosting; personal packages define experience.

**Status:** the [Rust trusted-host API](public/rust/README.md) and
[Chromium implementation](chromium/) are verified on macOS arm64 through a real
Content/AppKit window: load and paint, back/forward navigation, close,
history-boundary rejection, and closed-ID rejection. A persistent HTTPS window
and normal application quit also ran successfully. See the
[build evidence and development limitations](chromium/BUILDING.md#verified-native-mvp-results).
The MVP uses one disposable context and does not implement or qualify the
broader design below. The CEF plan is superseded by
[ADR 0001](../docs/decisions/0001-own-chromium-embedding.md).

The [first-version design](DESIGN.md) specifies the CEF-inspired object model,
proposed engine API, scoped decisions, native hosting, and lifecycle semantics
against the pinned Chromium sources. It is a proposal, not implemented code.

## Dependency and trust boundary

- [Core](../core/README.md) owns engine-neutral ports, stable identities, authorization, and observable operation semantics. It dispatches to an injected implementation without importing Chromium headers or this module's internals.
- This module implements that boundary using Chromium's public embedder interfaces. Objects such as `content::WebContents`, `content::BrowserContext`, and `content::StoragePartition` are private implementation details, not public Pliant identities.
- The trusted platform bootstrap wires the implementation into core and provides native facilities through narrow interfaces. The embedder must not depend on platform-module implementations, UI code, presets, or plugins.
- UI, plugins, and agent clients use core operations, not a second Chromium or DevTools command API. Page content and engine events never grant core authority.
- The embedder owns Chromium-backed content surfaces; the platform host owns native windows/containers and lends registered container leases for attachment. Surface references do not expose raw Chromium or OS handles and are not permanent page identities or authorization grants.

Runtime calls and source dependencies differ: core can invoke its engine port
without depending on the Chromium implementation. See the
[module map](../MODULES.md) for the allowed dependency directions.

## What this module owns

| Area | Embedder responsibility | Responsibility elsewhere |
| --- | --- | --- |
| Runtime | Implement the Content application/process entry points and embedder clients, initialize required resources/services, marshal work to the correct threads, and sequence shutdown. Preserve upstream sandbox and process-security mechanisms. | Platform supplies OS bootstrap, native event-loop facilities, helper packaging, and signing. Core coordinates application shutdown. |
| Pages | Own engine page instances and delegates/observers; translate navigation, loading, frame/document changes, close/unload, and failures. | Core owns stable identities, authorized dispatch, lifecycle semantics, and recovery decisions. UI organization and application workflows are not engine state. |
| Profile-backed storage | Wire core-selected profiles to browser contexts, storage partitions, and network/storage services. Use Chromium's cookies, cache, and site-storage implementations; expose scoped maintenance and release. | Core owns logical profile identity, access grants, allocation ownership, retention-policy enforcement, deletion coordination, and durable recovery state. |
| Web-content presentation | Own content surfaces backed by Chromium's native views, including attachment, geometry, scaling, visibility, input, IME, and accessibility integration. Evaluate offscreen composition only if required. | Platform owns native windows/containers and OS facilities, lending containers to the embedder. UI owns surrounding layouts. Core validates page targets and activation authority. |
| Decisions | Connect popup, permission, dialog, upload/download, and applicable authentication hooks to scoped host requests. Apply validated decisions and report outcomes. | Core coordinates authority and policy. Permissioned services supply replaceable policy; platform supplies native facilities. Engine defaults must not silently decide product policy. |
| Failures and diagnostics | Identify affected instances, invalidate references, classify errors, and report bounded diagnostic/resource information. | Core owns operation outcomes, retry authorization, and recovery. Higher-layer attribution stays outside the engine; tools inspect evidence, not own the recovery path. |
| Upstream integration | Own the source revision, Chromium-specific build integration, component/dependency review, patch inventory, and engine-data compatibility evidence. | Platform owns OS distribution/update integration; core coordinates migration, backup/restore, and activation. Tests supply cross-version evidence. |

Blink, V8, networking, storage, graphics, and sandbox primitives remain upstream
implementations. Writing an embedder does not mean rewriting those systems.
Conversely, the Content API is not a complete set of browser services: every
extra component needs explicit wiring and an ownership decision.

## Shared boundaries that must remain explicit

### Storage mechanism versus profile authority

A Pliant profile is not a Chromium directory, pointer, or OS process. Establish
and test its mapping to browser contexts and storage partitions; creating two
objects alone is not isolation evidence. Unrelated profiles must not share
backing data accidentally.

A live engine page must not silently change its profile binding. An unavailable
profile is a defined failure, not permission to use a default account. Profile
deletion, backup, and upgrade require engine-aware quiescence/release and actual
completion reporting; do not copy or remove live engine files and assume they
are consistent. Arbitrary engine-data downgrade is not promised.

### Native hosting versus browser UI

The embedder owns the content surface that works directly with Chromium's
native view. The platform host owns the surrounding OS window/container and
lends a `NativeContainerLease`; the embedder does not acquire window ownership.
`SurfaceRef` identifies the live attachment and is invalidated on detach, page
closure, or surface loss. Detaching alone preserves the live page. Coordinate
event-loop scheduling, input transport, and teardown without exposing Chromium
types to UI or core. See the [surface model](DESIGN.md#content-surface-versus-native-container).

Page creation, navigation, rendering visibility, input targeting, and foreground
activation are separate concerns. Background work must not activate a native
window or change OS focus as a side effect. If human interaction is required,
report a pending handoff rather than displaying an unsolicited native modal.
Native input may reach a core-bound surface directly, but must not bypass that
binding or allow arbitrary page targeting.

### Engine callbacks versus authorized decisions

Bind callbacks to exact page/profile identities and engine-instance/document
generations where relevant. Reject stale, expired, duplicate, or invalidated
decisions. Keep Chromium callbacks private. Core validates current authority;
the embedder validates that the underlying request is still live.

Not every callback can wait for a plugin or a person. Document synchronous and
asynchronous decision points for the pinned source revision. If a decision
cannot be obtained safely, deny/cancel and report why instead of silently
granting permission or blocking a critical engine thread.

Core authorization never bypasses origin checks, TLS validation, or sandboxing.
Credentials, cookies, tokens, raw profiles, and sensitive request/page data
must not leak into general UI state or diagnostics.

## Contract surface to specify before implementation

| Direction | Information crossing the boundary |
| --- | --- |
| Core to engine | Open/release an explicitly selected profile; create/close a page; navigate/control loading; perform supported page interactions; resolve scoped requests; cancel work; perform storage maintenance. |
| Platform host to/from engine | Startup/shutdown hooks, event-loop scheduling, content-surface attachment, bounds, scaling, visibility, input/IME transport, accessibility, and explicit activation. |
| Engine to core | Page/document state, navigation outcomes, download/storage progress, requests requiring a host decision, and failures with exact affected targets. |
| Engine to trusted diagnostics | Upstream/build identity, tested capability evidence, classified failures, and resource measurements subject to data-access/redaction rules. |

These are conceptual groups, not frozen function signatures or a public copy of
the Content API. Distinguish acceptance from completion, navigation commit from
load completion, and requested cancellation from confirmed cancellation.
Preserve unload confirmation; define duplicate close and late-callback behavior.
Renderer failure may affect several pages. Recovery and retries must not imply
rollback or exactly-once execution of a website's external effects.

## Explicit non-responsibilities

- Profile selection, account-routing rules, grants, plugin registration, and plugin isolation.
- UI organization, application workflows, session-restore policy, bookmarks, address suggestions, or the browser's global history model. Per-page navigation history is an engine mechanism, not the global model.
- Credential-provider policy, passkey storage, or OS credential facilities. Engine-side credential integration requires separate feasibility evidence and core authorization.
- Browser settings UI, Chromium's browser interface, extension compatibility, or an unrestricted Content/DevTools API for plugins and agents.
- The UI language, native shell layout, installers, signing, update delivery, or the trusted recovery interface.

DOM observation, script execution, network interception, capture, and debugging
need explicit permissioned Pliant contracts. They do not become unrestricted
public features merely because an upstream API exists.

## First implementation sequence

1. Provision and validate the pinned source workspace using [chromium/BUILDING.md](chromium/BUILDING.md). Keep source validation, dependency synchronization, compilation, and runtime evidence separate.
2. Add the smallest native Content bootstrap and content-view host on macOS arm64. Use upstream C++ embedder interfaces; keep native glue private. Do not decide the portable core language or UI framework by accident.
3. Wire isolated persistent contexts and real page lifecycle events to the initial engine-neutral contracts. Add controlled fixtures and negative tests.
4. Demonstrate background operation without focus theft, host-controlled requests, renderer-failure reporting, and safe teardown.
5. Repeat the hard-boundary scenarios on Linux and Windows, then test a second Chromium revision before claiming a sustainable embedding boundary.

The [capability matrix](../docs/contracts/engine-capabilities.md) defines the
broader evidence needed. Its complete scenarios remain unverified; neither the
narrow MVP smoke nor repository checks establish those guarantees.
