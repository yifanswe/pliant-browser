# Rust embedder MVP

One disposable browsing context and a plain macOS content window. No toolbar,
sidebar, profile UI, plugin system, or CDP connection. This is a trusted-host
library, not the public customization API from the broader design.

**Verified on 2026-09-18:** the complete framework and Rust hosts built on macOS
arm64; the native navigation smoke passed twice, and the persistent HTTPS window
painted and accepted normal application quit. See
[build evidence and known development diagnostics](../../chromium/BUILDING.md#verified-native-mvp-results).
The trusted-host container and asynchronous beforeunload seam described below
is source-reviewed, but native-runtime verification remains pending.

## API

Call `run(handler)` once on the main thread. In the handler, use:

- `create_page(url)` to create a page, display its native test window, and load a URL.
- `create_page_in(container, url)` to create a page in a trusted host-owned
  `NSView` without creating another window.
- `load_url(page, url)` to navigate the same instance.
- `back(page)` and `forward(page)` to traverse its real Chromium history.
- `attach_page(page, container)` and `detach_page(page)` to move a live page
  between trusted host layouts without navigating or recreating it.
- `close_page(page)` to request asynchronous beforeunload authorization.
- `shutdown()` to end the disposable host session.

Navigation commands acknowledge dispatch; `Event::Navigated` reports a committed
primary-frame navigation. `Event::Painted` reports a first non-empty paint while
the window is visible. Neither means network idle. Failures and destruction have
separate events. Commands run in main-thread callbacks or a synchronous
`with_engine` scope entered by a native main-thread host callback; `Engine` and
`NativeContainer` are not `Send` or `Sync`.

Normal close auto-cancels a genuine confirmation request and emits
`Event::CloseRefused`; otherwise `Event::Closed` reports destruction. Command
acceptance is not destruction, so retain the host container until detach
succeeds or `Closed` arrives. Ordinary `pagehide`/`visibilitychange` listeners
do not cause refusal. A page ID is invalidated only after `CloseContents`. Host
shutdown is explicit disposal of the whole session. Unsupported permission
requests, popups, and downloads are denied. The engine keeps Chromium's sandbox;
remote-debugging switches are rejected.

## Native build

The Rust library links the **PliantContent framework** produced by
[the Chromium GN target](../../chromium/BUILD.gn), not a prebuilt Chrome
application or CEF.
The framework implements a small C ABI over Content; macOS helper executables
initialize Chromium's sandbox before loading it.

The first target is macOS arm64. See
[build prerequisites](../../chromium/BUILDING.md). In the prepared Chromium
source checkout, expose this repository's `embedder/` as `//pliant_mvp`,
configure a component build, then generate and compile:

```sh
gn gen out/pliant-mvp \
  --root-target=//pliant_mvp/chromium \
  '--root-pattern=//pliant_mvp/chromium:*'
third_party/ninja/ninja -C out/pliant-mvp -j 4 \
  pliant_mvp/chromium:pliant_content
```

Development configuration: `target_os="mac"`, `target_cpu="arm64"`,
`is_debug=false`, `is_component_build=true`, `symbol_level=0`,
`blink_symbol_level=0`, `v8_symbol_level=0`, `use_system_xcode=true`,
`use_lld=false`, `use_remoteexec=false`, `use_siso=false`,
`chrome_pgo_phase=0`, `use_thin_lto=false`.
Apple's linker is selected because the pinned LLVM linker cannot parse the
macOS 27 SDK's new target metadata. No SDK files are patched.

## Independent test app

The manual host, navigation/window/threading examples, diagnostic logging, and
safe non-overwriting package recipe live under
[`apps/embedder_test/`](../../../apps/embedder_test/README.md). They depend on
this crate through its public Cargo path.

The current component-build packages are not standalone: matching Chromium
component dylibs remain outside each app under `PLIANT_CHROMIUM_LIB_DIR`.
Packages must remain direct children of that matching output directory and
cannot be moved elsewhere without repackaging those runtime dependencies.

The navigation example serves synthetic loopback pages and checks **load and
paint A → load and paint B → back to A → forward to B → close**, including
history-boundary and stale-page rejection. Native compilation and real run
paths remain separate acceptance gates; `cargo check`, Clippy, and formatting
alone are not browser execution.
