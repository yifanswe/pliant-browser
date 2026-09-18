# Native customization demo

This macOS app composes the `pliant-embedder` trusted-host API with
`pliant-ui-definition`. The AppKit renderer walks the typed `Node` tree; it
does not branch on preset names. The fixed path, Preview, Apply, Reject, and
Restore Default controls live outside the replaceable tree.

Preview launches a separate copy of the executable with an owned immutable
snapshot and disposable Chromium context. A candidate-bound acknowledgement is
accepted only after that child validates the exact snapshot, initializes the
engine and native host, renders the layout, and reaches the event loop. Apply
stays disabled while the child is starting and rejects child failure, exit,
timeout, replacement, or a mismatched acknowledgement.

Apply uses that same in-memory candidate, then atomically stores its exact
source. Restore Default replaces only the layout and address-open policy; live
pages are reattached without navigation or recreation. If persistence fails,
the live layout remains active, controls are refreshed, and Retry Save retains
the exact active bytes while warning that restart will load the previous saved
state.

The checked-in source can be tested and type-checked without a Chromium
framework:

```sh
PLIANT_BROWSER_TEST_TMPDIR=/tmp/pliant-browser-tests \
  cargo test --offline --locked --manifest-path apps/browser/Cargo.toml
cargo check --offline --locked --manifest-path apps/browser/Cargo.toml \
  --features native-runtime --bin pliant-browser
```

Running requires a matching, newly rebuilt `PliantContent.framework` containing
the host-container ABI used by this source. With that artifact:

```sh
export PLIANT_CHROMIUM_ROOT=/path/to/pliant-chromium
export PLIANT_CHROMIUM_LIB_DIR="$PLIANT_CHROMIUM_ROOT/src/out/pliant-mvp"
bash apps/browser/run_macos.sh \
  --package-only \
  --app-path "$PLIANT_CHROMIUM_LIB_DIR/PliantBrowserDevelopment.app" \
  --bundle-id org.pliant.browser.development \
  --bundle-name "Pliant Browser Development"

"$PLIANT_CHROMIUM_LIB_DIR/PliantBrowserDevelopment.app/Contents/MacOS/pliant-browser" \
  --definition "$PWD/presets/classic/definition.json"
```

This is a component-build development package, not a standalone app. Chromium
component dylibs remain outside the bundle in `PLIANT_CHROMIUM_LIB_DIR`; keep
the app as a direct child of that matching build output and do not move it
elsewhere.

`--fixture-setup` starts loopback acceptance fixtures at `/a`, `/b`,
`/confirm`, and `/events`; `--e2e` remains a compatibility alias and does not
run or claim an end-to-end test. Page A contains editable form state,
cookie/local-storage markers, and observable ordinary `pagehide` and
`visibilitychange` events. `/confirm` installs a dirty `beforeunload` handler
after user input and provides an explicit way to allow a later close. Preview
children receive the parent's exact loopback URL, so main and preview can be
checked at the same origin while their separate Chromium contexts must keep
cookies and storage isolated.

## Post-migration native acceptance checklist

Use a newly rebuilt matching framework and a uniquely named package. Launch
without `DYLD_*` overrides:

```sh
bash apps/browser/run_macos.sh \
  --package-only \
  --app-path "$PLIANT_CHROMIUM_LIB_DIR/PliantBrowserAcceptance.app" \
  --bundle-id org.pliant.browser.acceptance \
  --bundle-name "Pliant Browser Acceptance"

"$PLIANT_CHROMIUM_LIB_DIR/PliantBrowserAcceptance.app/Contents/MacOS/pliant-browser" \
  --fixture-setup \
  --definition "$PWD/presets/classic/definition.json"
```

Record screenshots or tester notes and measured edit-to-ready latency for each
step; fixture setup alone is never a pass:

1. Exercise Classic, Workspace, and
   `apps/browser/fixtures/third-definition.json`; verify visibly different
   layouts and current-page versus new-page address behavior.
2. Type in `/a`, navigate to `/b` and back, then Preview/Apply/Restore. Verify
   page identity, form value, history, and the exact previewed candidate survive
   layout replacement. Reject a valid candidate and an invalid edit; verify the
   active browser is unchanged and recovery controls remain usable.
3. Close `/a` after resetting the lifecycle log; verify it closes and `/events`
   records ordinary lifecycle delivery. Type into `/confirm`, request Close,
   verify the explicit refusal status and retained form/page, choose Allow
   Close, then verify a second request produces `Closed`.
4. Set the storage marker in main `/a`, preview a candidate, and verify the
   preview's `/a` uses the identical origin but sees empty cookie/local storage.
5. Force a save failure for both Apply and Restore, verify the new live controls
   remain usable with the active-but-unsaved warning, restore write access, and
   use Retry Save without previewing/applying again.
6. Inspect the packaged browser executable with `otool -l` for
   `@executable_path/../Frameworks`, then launch both main and preview without
   environment search-path overrides.
