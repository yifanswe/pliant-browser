# Embedder test app

This package is an independent trusted-host consumer of `pliant-embedder`. It
owns the minimal AppKit manual controls, loopback fixtures, startup diagnostics,
test-app metadata, and the legacy navigation, window, and threading examples.
It does not own the Chromium implementation or the public ABI.

The manual app exposes create/load A/load B/back/forward/close/stale-ID controls
through the same Rust API used by other trusted hosts. `--diagnostic-log PATH`
or `PLIANT_EMBEDDER_DIAGNOSTIC_LOG` records Rust and native startup/lifecycle
events without changing app behavior.

Run its source tests with outputs outside the repository:

```sh
export PLIANT_CHROMIUM_ROOT=/path/to/pliant-chromium
export PLIANT_CHROMIUM_LIB_DIR="$PLIANT_CHROMIUM_ROOT/src/out/pliant-mvp"
export CARGO_TARGET_DIR="$PLIANT_CHROMIUM_LIB_DIR/rust-embedder-test"
mkdir -p "$CARGO_TARGET_DIR/debug/Frameworks"
ln -sfn "$PLIANT_CHROMIUM_LIB_DIR/PliantContent.framework" \
  "$CARGO_TARGET_DIR/debug/Frameworks/PliantContent.framework"
cargo test --offline --locked --manifest-path apps/embedder_test/Cargo.toml
```

Package without replacing or launching an existing app:

```sh
bash apps/embedder_test/package_macos.sh \
  --package-only \
  --app-path "$PLIANT_CHROMIUM_LIB_DIR/PliantEmbedderTestDevelopment.app" \
  --bundle-id org.pliant.embedder-test.development \
  --bundle-name "Pliant Embedder Test Development"
```

This is a component-build development package, not a standalone app. Chromium
component dylibs remain outside the bundle in `PLIANT_CHROMIUM_LIB_DIR`; keep
the app as a direct child of that matching build output and do not move it
elsewhere.

Pass `--example navigation`, `--example window`, or `--example threading` to
package one of the linked examples instead of the manual app. The output path,
bundle identifier, and bundle name are required; an existing bundle is never
overwritten. Without `--package-only`, arguments after `--` are passed to the
packaged executable.

These are ad-hoc component-build packages, not portable or notarized
distribution artifacts. Native compile/link, signature verification, and actual
GUI interaction remain separate from Rust unit tests.
