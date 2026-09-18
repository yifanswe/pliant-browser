# ADR 0001: Own the Chromium embedding layer

**Status:** accepted architectural direction, 2026-09-17. Implementation and
cross-platform feasibility remain unverified.

## Decision

Pliant will build its own embedder over Chromium's Content API and explicitly
selected supporting components. This supersedes the earlier CEF-backend plan.
We are not forking the full Chromium browser application or adopting its tabs,
settings UI, extension model, and product policies.

The implementation belongs in [engine/chromium/README.md](../../engine/chromium/README.md).
Its public boundary remains the engine-neutral contracts owned by core, not
Chromium's C++ object model.

## Why

The project's owner wants the browser foundation itself to be malleable. We
choose to own the integration points for page lifecycle, profile-backed storage,
native hosting, and host decisions rather than make CEF's exposed callbacks the
limit of that integration.

This is a decision to accept integration and maintenance responsibility, **not
evidence that CEF cannot support a malleable browser**. No feasibility test has
established that direct embedding is faster, smaller, or safer. Malleability
still depends on the quality of Pliant's public contracts and runtime guards.

## Reuse versus ownership

| Reuse from Chromium | Own in Pliant |
| --- | --- |
| Blink, V8, networking, web storage, graphics, process and sandbox mechanisms | Runtime bootstrap, native hosting integration, lifecycle translation, and safe shutdown |
| Content API objects and embedder hooks | Mapping Pliant page/profile identities to engine objects without exposing those objects |
| Selected services and components where appropriate | Wiring decisions through core authorization and permissioned service contracts |
| Upstream source and tests | Pinned builds, adaptation to upstream changes, embedding regression coverage, and timely security updates |

Do not reimplement cookie security, the network stack, the renderer, or sandbox
primitives. Owning the embedder is not permission to weaken Chromium's security
boundaries or to give plugins raw engine access.

## Dependency boundary

- Use the supported source-level Content embedder interfaces; do not spread dependencies on Chromium implementation headers throughout Pliant.
- Do not base the product on the full Chromium browser target or inherit its browser-product model. Review each additional component, dependency, license, service requirement, and default policy before adopting it.
- Use Chromium's content shell as an integration reference and possible toolchain smoke test, not the shipped browser or a production security configuration.
- If a necessary change goes beyond public embedder interfaces, isolate it, record its rationale and affected tests, and track its upstream status and rebase cost.
- Keep Chromium source and build output in a separately provisioned workspace. Do not vendor the checkout into this repository or execute a large checkout automatically.

The first engine integration will work with Chromium's C++ interfaces, with
Objective-C++ at macOS-specific seams where necessary. This does **not** select
the language of core, UI, plugins, or the eventual Rust/C++ interoperability
boundary. Those decisions still require evidence.

## Costs accepted

We take on work CEF would otherwise supply: process/bootstrap integration,
native content hosting, context and service wiring, callback lifetimes, a
language boundary if needed, Chromium-specific tests, and source-build upkeep.

Content's public interfaces are source-level interfaces, not a separately
shipped stable embedding SDK. An ordinary prebuilt Chromium browser does not
provide a supported set of Content headers and linkable libraries for Pliant.
Plan for a matching Chromium source/dependency/toolchain build.

Pliant must continue to version its own contracts independently. Updating
Chromium may require recompiling or adapting the embedder even when a personal
package remains unchanged. A delayed security update is not an acceptable way
to preserve a customization.

## Alternatives considered

- **CEF:** mature embedding integration, distributions, and a C/C++ API boundary. Rejected as the current backend choice because we elect to own that integration, not because Chrome UI is mandatory in CEF.
- **Full Chromium browser fork:** outside the requested architecture; it starts with a browser product rather than only its infrastructure.
- **New web engine:** outside scope. We reuse Chromium's implementations.

## First evidence and stop conditions

Start with the [source-build preparation](../../engine/chromium/BUILDING.md) and
[capability matrix](../contracts/engine-capabilities.md). A source pin and build
instructions are not a working embedder.

The first native spike must show a real web-content view, two persistent profile
contexts, non-activating background work, host-controlled decisions, and correct
teardown with the sandbox enabled. Start on macOS arm64, the current development
host; Linux and Windows remain required product targets, not claimed support.

Revisit the approach if maintaining the required boundaries or shipping timely
Chromium updates proves unsustainable. Do not silently add CEF as a fallback,
substitute the full browser, or disable sandboxing to satisfy a demo.

## References

- [Chromium Content module](https://chromium.googlesource.com/chromium/src/+/main/content/README.md)
- [Content public interfaces](https://chromium.googlesource.com/chromium/src/+/main/content/public/README.md)
- [Chromium content shell](https://chromium.googlesource.com/chromium/src/+/main/content/shell/)
- [Chromium Rust integration](https://chromium.googlesource.com/chromium/src/+/main/docs/rust/README.md)
