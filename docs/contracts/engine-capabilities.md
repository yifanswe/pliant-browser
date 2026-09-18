# Engine capability evidence

**Status:** draft acceptance requirements for the Pliant-owned Chromium
embedder. The narrower Rust/macOS MVP has
[real native load/paint/history/close evidence](../../embedder/chromium/BUILDING.md#verified-native-mvp-results),
but none of the complete acceptance scenarios below has been qualified. This is
not a frozen SDK. The [decision](../decisions/0001-own-chromium-embedding.md)
selects the integration approach; it is not itself runtime evidence.

## Evidence rules

- **Unverified:** the complete scenario has not been qualified with real-engine/platform evidence; narrower partial evidence may exist.
- **Tested:** the scenario passed for the named revision, build, OS, architecture, and display configuration, with reproducible evidence.
- **Unsupported:** the named configuration cannot satisfy the requirement; record why and the user-visible failure. Never silently substitute another account or privileged service.
- A failing run remains failing evidence, not a reason to label a requirement supported. Link the failure and track the correction.

Every tested entry must identify the source pin, dependency/patch state, build
arguments, machine/OS, fixture revision, procedure, observed outcome, and evidence
location. Unit/model tests and checkout validation cannot establish native
focus, storage isolation, or sandbox behavior.

## First vertical slice

| ID | Observable requirement | Required negative/failure case | macOS arm64 | Linux | Windows |
| --- | --- | --- | --- | --- | --- |
| ENG-001 | Bootstrap a native host with a real web-content view and upstream sandbox enabled; resize and close it cleanly. | Startup/resource failure is reported; no fallback to a full Chromium browser or weakened sandbox. | Unverified | Unverified | Unverified |
| ENG-002 | Page creation and navigation report acceptance, creation, commit/load outcomes, and failure separately. | Missing/stale page IDs and late callbacks cannot affect replacement instances/documents. | Unverified | Unverified | Unverified |
| ENG-003 | Two profiles keep distinct cookies and site storage, including after restart. | Deliberately sharing backing storage causes the isolation test to fail; missing profiles never use a default context. | Unverified | Unverified | Unverified |
| ENG-004 | While a human edits in A, an authorized client creates and interacts with B without changing A's selected page, window focus, keyboard focus, or form state. | An operation needing foreground UI enters a pending handoff instead of stealing focus. | Unverified | Unverified | Unverified |
| ENG-005 | Popups, site permissions, uploads/downloads, and dialogs follow a scoped host decision with actual completion reporting. | Denied, expired, revoked, duplicate, or document-invalidated decisions cannot authorize work. | Unverified | Unverified | Unverified |
| ENG-006 | Close honors required unload confirmation; renderer failure reports every affected page instance. | Repeated close, unload refusal, cancellation, and late callbacks have defined outcomes; recovery does not replay unsafe actions. | Unverified | Unverified | Unverified |
| ENG-007 | Keyboard, IME, accessibility, scaling, and content-surface lifetime work through the real native path. | Detaching/destroying a surface invalidates input targeting and releases native/engine references safely. | Unverified | Unverified | Unverified |
| ENG-008 | Profile maintenance and a second Chromium revision preserve documented data/operation behavior. | Interrupted release/upgrade is recoverable on synthetic copies; binary rollback is not assumed safe for migrated data. | Unverified | Unverified | Unverified |

Linux display-server variants and both Linux/Windows CPU coverage must be named
when evidence is added. The macOS column names only the first development
configuration; other supported architectures need their own evidence.

### Primary scenario: human A, background task B

1. Create two synthetic persistent profiles and confirm separate storage. The human edits a controlled form in A.
2. A deterministic authorized client requests a page in B through core. Core validates profile access and assigns page/operation identities; the embedder creates only that target without foreground activation.
3. Load and interact with a controlled fixture in B. Record actual native focus, selected page, and A's form state, not only successful protocol responses.
4. Trigger a permission or file request. It waits for an authorized decision/handoff, or fails safely if asynchronous deferral is unavailable.
5. Revoke the client grant or hand the page to the human. Core blocks future unauthorized operations; pending work reports what actually completed or was cancelled.
6. Cleanup affects only authorized pages. Restart and verify profile separation and persistent data. Do not promise rollback of website effects already accepted remotely.

Use synthetic fixtures only. The embedder's profile/page machinery is not the
authorization owner: a test client must not gain authority from possessing a
Chromium pointer, a renderer message, or a correctly typed plugin result.

## Additional capability investigations

Credential/passkey integration, PDF/printing, media/codecs, clipboard, capture,
DOM observation, script execution, and network interception require separate
permissioned contracts and dependency reviews. They are currently unverified,
not implicitly supplied by adopting Content. Record service/licensing limits
without treating missing features as permission to bypass a boundary.

Measure cold startup, idle CPU, whole-process-tree memory, runtime/package disk
footprint, and a fixed multipage workload once the first real build runs. There
are no measured performance budgets or efficiency claims yet.
