# Preparing the Chromium source build

**Current state:** the macOS arm64 [Rust host API](../public/rust/README.md) has been built and
executed against the pinned Chromium Content implementation. The complete
framework, Rust hosts, resources, and all three sandbox helpers link and package.
The real native navigation/paint/history/close smoke passed twice; the independent
plain-window example loaded and painted HTTPS content and accepted normal
application quit. This verifies the narrow MVP, not the broader engine contract.
See [verified native results](#verified-native-mvp-results) and the development
runtime diagnostics below.

After the repository-layout migration, the mapped
`//pliant_mvp/chromium:pliant_content` target rebuilt and linked incrementally
against the same cached dependencies and upstream pin. The resulting packages
still require independent post-migration GUI acceptance; compilation and
packaging do not substitute for that run.

## Consuming versus producing the native library

The [Rust build](../public/rust/build.rs) links the `PliantContent` framework from
`PLIANT_CHROMIUM_LIB_DIR`. Once a matching native runtime artifact is available,
Rust callers can build against it without a Chromium source checkout. The
runtime's required resources and helper-process setup must accompany the
library; a library search path alone does not package an application.

Pliant's native target is the producer of this library. It needs a matching
Chromium source/dependency/toolchain build. No current official prebuilt Content
SDK matching this bridge has been identified. The old
[libchromiumcontent distribution](https://github.com/electron/libchromiumcontent)
is deprecated, and the upstream
[headless-shell download](https://chromium.googlesource.com/chromium/src/+/main/headless/README.md)
is an executable, not this library's ABI. Substituting CEF or controlling an
executable through DevTools would be a different backend choice.

## Local build readiness

Verified on 2026-09-17 after the Xcode installation and external disk attachment:

- Xcode 27.0, build 27A266a, is selected as the developer toolchain.
- The macOS 27.0 SDK version, SDK path, and platform-path queries succeed.
- The user installed Apple's separate Metal Toolchain after ANGLE's shader build reported it missing. Both `xcrun --find metal` and `xcrun metal --version` now succeed; the compiler reports `32023.921 (metalfe-32023.921.6)`. This prerequisite is resolved.
- The previous Command Line Tools discovery failures are resolved. The first compile found that the pinned LLVM linker cannot parse the SDK 27 target metadata. The build now uses the supported `use_lld=false` option to select Apple's linker; SDK contents are unchanged.
- The external `photo4` volume is writable APFS over USB, with over 660 GiB free when measured during the build. Dependency scans on this volume have taken several minutes; a sampled slow scan was waiting in filesystem metadata reads, not for user input. Preserve incremental outputs and avoid unnecessary build restarts.

The selected source/build workspace is external to this repository and is
identified by `PLIANT_CHROMIUM_ROOT`. A full shallow source checkout and pinned
dependencies are present there,
along with the Chromium compiler, GN, and Ninja. The original inspection
checkout remains on the internal disk. An external copy of that inspection
checkout was preserved separately rather than overwritten.

The headless draft has been replaced with a direct Content/AppKit implementation
and a plain-window Rust example. The complete `PliantContent` framework and both
Rust examples now run in a development app beside the matching component
libraries. Generated files and native build outputs stay on the external volume;
no existing personal data was modified or reformatted.

## Verified native MVP results

Verified on **2026-09-18**, macOS **26.6.2**, **arm64**, using Xcode **27.0**,
SDK **27.0**, the pinned Chromium compiler/Ninja, and the component-build arguments
in the [MVP build instructions](../public/rust/README.md#native-build). The fixture and bridge
are the local MVP sources; this is not a published runtime artifact.

| Check | Observed result |
| --- | --- |
| Native build | `//pliant_mvp/chromium:pliant_content` completed, including the full Content framework, resource pack, V8 snapshot, and default/Renderer/GPU helpers. |
| Rust/native boundary | The Rust hosts linked; the final framework exports the functions declared by [the C ABI](../public/c/bridge.h). |
| Development packaging | Recursive strict code-signature verification passed for the app, framework, and nested helpers. These are ad-hoc development signatures, not distribution signing or notarization. |
| [Navigation smoke](../../apps/embedder_test/examples/navigation.rs) | Two fresh-process runs passed: load and paint A, load and paint B, back to A, forward to B, close, then reject every command using the closed ID. Unavailable back/forward entries were also rejected. |
| [Persistent window](../../apps/embedder_test/examples/window.rs) | `https://example.com/` committed and produced a visible-content paint callback. macOS reported one on-screen native window owned by the Pliant process. |
| Teardown | Normal application quit completed. After the smoke and window runs, no Pliant host or helper processes remained. |

These runs used the sandbox-first helpers and no remote-debugging endpoint,
single-process mode, sandbox bypass, or certificate bypass. Presentation evidence
is Chromium's real first-nonempty-paint callback plus macOS window metadata, not
external screenshot/pixel inspection. Screen-recording and Accessibility access
were not granted or bypassed.

### Development runtime diagnostics

- The default static-ANGLE component build logs duplicate registration of the upstream `ANGLESwapCGLLayer` class from Blink and GL component libraries. The tested pages rendered successfully, but this warning remains a development-build limitation; it is not proof that every graphics path is safe for release.
- Shutdown logged an already-missing child process (`ESRCH` from Chromium's `SIGTERM` path); one repeat also logged rejected macOS task-priority/suppression updates. The smoke still returned success and process inspection found no remaining Pliant processes. These diagnostics are preserved, not suppressed or counted as new capability evidence.

Input/IME, explicit resize behavior, persistent/profile isolation, background
focus, broader decision handling, crash recovery, upgrades, and other platforms
are not qualified by this smoke. The broader
[capability matrix](../../docs/contracts/engine-capabilities.md) remains
unverified.

## Source inspection checkout

For the [v0 design](../DESIGN.md), a source-only checkout was completed on
2026-09-17 in the sibling workspace `pliant-chromium/src` on the internal disk. It is shallow
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

This is a reproducible **feasibility baseline**, locally verified only for the
MVP configuration above, not an endorsed production release. Before release,
select a supported, security-current revision and run the full upgrade/capability
suite. Do not follow a moving branch implicitly or assume a source pin proves
dependency synchronization.

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

Source and dependency provisioning use the upstream tools in the selected
external workspace. Cargo's Rust type-check/lint operations do not download or
build Chromium; native build tasks are separate and explicit.

## Build entry point and remaining qualification

Expose this repository's `embedder/` directory in the prepared checkout through
the project-owned `//pliant_mvp` source-root mapping. Generate only the Chromium
integration target's dependency graph with
`--root-target=//pliant_mvp/chromium` and
`--root-pattern=//pliant_mvp/chromium:*`; this avoids generating unrelated
browser/test targets. Use the checkout's pinned Ninja consistently, not a
different system version that rewrites its build log. Beyond the minimal flow
above, qualify these native integration points:

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
