# Preparing the Chromium source build

**Current state:** there is no Pliant Content embedder build target yet. This
document records the source baseline and build prerequisites; it does not build
or launch a browser.

## Source inspection checkout

For the [v0 design](DESIGN.md), a source-only checkout was completed on
2026-09-17 in the external sibling workspace `pliant-chromium/src`. It is shallow
and blob-filtered, with a sparse working tree containing Content public
interfaces, the content shell, documentation, permission/content-setting and
download interfaces, network public interfaces, and sandbox policy sources.

HEAD matches the pinned commit below and the tracked working tree was clean.
The local checkout occupied approximately 208 MiB at inspection time. It contains
**no synchronized DEPS dependencies, installed Chromium toolchain, generated
build files, or compiled browser**. Sparse source inspection is not a build-ready
checkout; expand/provision the workspace and follow upstream synchronization
before implementation or compilation.

## Upstream baseline

[upstream.json](upstream.json) pins Chromium **152.0.7977.42** at commit
`db8ceb709fe92f3bb010fb982d6300e54de6dc6a`. The
[official tag](https://chromium.googlesource.com/chromium/src/+/refs/tags/152.0.7977.42)
and its version metadata were checked on 2026-09-17.

This is a reproducible **feasibility baseline**, not an endorsed production
release or a verified build. Before release, select a supported, security-current
revision and run the full upgrade/capability suite. Do not follow a moving branch
implicitly or assume a source pin proves dependency synchronization.

## Provision outside this repository

Use the matching upstream instructions for
[macOS](https://chromium.googlesource.com/chromium/src/+/db8ceb709fe92f3bb010fb982d6300e54de6dc6a/docs/mac_build_instructions.md),
[Linux](https://chromium.googlesource.com/chromium/src/+/db8ceb709fe92f3bb010fb982d6300e54de6dc6a/docs/linux/build_instructions.md), or
[Windows](https://chromium.googlesource.com/chromium/src/+/db8ceb709fe92f3bb010fb982d6300e54de6dc6a/docs/windows_build_instructions.md).
Start on macOS arm64; that is an initial development host, not a restriction on
the planned desktop targets.

- Allocate a source/build volume with room for the checkout, synchronized dependencies, toolchains, build outputs, debug symbols, and at least one upgrade comparison. Measure actual usage; a browser download size is not a build-workspace estimate.
- Acquire Chromium and its dependencies with the upstream tooling, checkout the pinned revision, and synchronize the matching dependencies and hooks. Keep personal browser profiles out of that workspace.
- Use the matching SDK/toolchain and Chromium's GN/Ninja build flow. A CMake-only project cannot turn an ordinary Chromium application binary into a Content API SDK.
- Preserve the upstream sandbox and process isolation. Do not use disable-sandbox or single-process shortcuts to claim a successful Pliant scenario.

No tool in this repository downloads Chromium, runs its hooks, checks out a
revision, changes an existing source tree, or installs toolchains automatically.

## Next build work

Once a source workspace is provisioned, add a Pliant GN target against the
pinned Content public interfaces. Verify these integration points against that
checkout before writing implementations:

- Content main delegate and browser/renderer clients; resource initialization and helper processes.
- Browser contexts, storage partitions, required delegates/services, and profile shutdown ordering.
- Native content-view hosting, input/IME, accessibility, focus, and clean teardown.
- Structured lifecycle/decision events into core, with no unrestricted renderer-to-host bridge.

The upstream content shell can demonstrate that the toolchain runs, but it is a
test embedder with test-oriented defaults. It is not Pliant's implementation or
evidence of our authorization, persistence, or security behavior. Do not build
the full Chromium browser target as a substitute for our embedder.

Record OS/architecture, compiler and SDK, GN arguments, synchronized dependency
state, patch set, build commands/results, and scenario evidence together. Use
the [capability matrix](../../docs/contracts/engine-capabilities.md); do not mark
a row tested from compilation alone.
