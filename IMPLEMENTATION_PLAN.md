# Implementation plan

[Overview](README.md) · [Design philosophy](DESIGN_PHILOSOPHY.md)

## Goal and status

Build infrastructure that lets users and their chosen coding agents create a personal browser. Deliver one or more ready-to-use browsers through the same public contracts.

**Design principle: flexibility of customization with safety guards.**

This is an implementation roadmap, not a record of completed browser work. The repository currently contains design documents, concept illustrations, a module-boundary scaffold, a pinned Chromium source baseline, and an existing repository structure check. It does not contain a working browser, Content embedder, native shell, DSL compiler, plugin runtime, or SDK.

The first proof is deliberately concrete:

> One Pliant-owned Chromium embedder behind core contracts, two substantially different interfaces, a replaceable account-routing service, and a human browsing while an agent works in another profile without stealing focus.

A second proof is equally important: upgrade the foundation while keeping an existing customization usable without an AI repair step.

## 1. Decisions and boundaries

| Decided | Meaning |
| --- | --- |
| Desktop first | Target Linux, macOS, and Windows. Mobile is outside the current implementation scope. |
| Own the Chromium embedder | Build on the Content API and selected supporting components, not CEF or the full Chromium browser application. Start with one backend behind Pliant contracts; see [ADR 0001](docs/decisions/0001-own-chromium-embedding.md). |
| Native browser UI | Do not use Electron. The native rendering framework remains unselected. |
| Complete customization | Support replacement of the whole UI and substantial browser services and policies, not just themes or fixed plugin slots. |
| Stable core | Custom code uses public contracts. It cannot patch core implementation or bypass core invariants. |
| No browser-extension compatibility | Do not implement Chrome/Firefox extension APIs or support installing their extension packages. This is an explicit non-goal, not deferred work. |
| Local coding agents | Support existing agents through ordinary developer tools. Do not require a built-in model or a new agent runtime. |
| Lightweight operation | Measure distribution footprint, memory, startup, and idle activity separately. Do not reduce isolation to improve a headline metric. |

The core language, native UI framework, DSL syntax, plugin execution technology, minimum OS versions, and supported CPU architectures remain open. The initial Content integration uses Chromium's C++ interfaces and platform glue as required; that does not choose the language of the portable core or a Rust/C++ boundary. Resolve remaining choices through the first feasibility stage.

### What Arc and Neo contribute to the plan

Arc is an experience reference for workspace organization, persistent versus temporary pages, keyboard navigation, and coherent browser UI. Neo is an experience reference for agent task contexts, background work, visibility into actions, and human supervision.

These references do not require cloning either product or inheriting its internal architecture. Pliant must make alternative experiences possible through the same platform.

## 2. Architecture to prove

Separate three responsibilities:

**Core mechanisms:** stable identity, permission enforcement, profile isolation, operation dispatch, lifecycle, resource accounting, and persistent-state integrity.

**Replaceable services and policies:** account routing, session organization, credential providers, download policies, and other behaviors exposed through explicit extension contracts.

**Personal interface:** complete layouts, local presentation state, input bindings, and interactions expressed through the UI language.

An official service may need privileges that a layout does not need. Those privileges must be declared and enforced through the same grant mechanism available to another authorized implementation. Official presets must not depend on undocumented shortcuts.

### Proposed repository boundaries

Product paths remain scaffolds; no product API is implemented or frozen.
Build integration for the Content
embedder will use the pinned Chromium source workspace, not a standalone
prebuilt browser as an SDK.

| Path | Responsibility |
| --- | --- |
| `docs/decisions/` | Architecture decisions and rejected alternatives. |
| `docs/contracts/` | Observable behavior, permissions, lifecycle, and compatibility promises. |
| [engine/chromium/README.md](engine/chromium/README.md) | Content embedding, selected service integration, upstream adaptation, and translation into Pliant contracts. |
| `core/` | Trusted identities, operations, policy enforcement, and state ownership. |
| `platform/` | Native windowing, OS credential facilities, packaging, and device integration. |
| `ui/` | DSL schema, validation, evaluation, and native rendering. |
| `plugins/runtime/` | Plugin isolation, grants, lifecycle, and service registration. |
| `plugins/reference/` | Official replaceable service implementations. |
| `presets/workspace/` | Workspace-oriented reference browser. |
| `presets/classic/` | Traditional tab-oriented reference browser. |
| `tools/` | Capability inspection, validation, preview, diagnostics, and packaging. |
| `tests/fixtures/` | Controlled websites, disposable profiles, and known migration inputs. |
| `tests/contracts/` | Tests of stable behavior shared by all presets. |
| `tests/browser/` | Real Chromium embedder and native-window scenarios. |
| `tests/security/` | Permission, isolation, and adversarial scenarios. |
| `tests/upgrade/` | Old-package compatibility, migrations, and recovery. |

Separate browser data from customization packages. A shared UI package must not contain profile cookies, passwords, browsing history, or private test artifacts.

## 3. Delivery sequence

### Stage 0: Build the Chromium embedder spike and choose the remaining stack

**Question:** Can the selected integration support the product's hard requirements on all three desktop platforms?

The [embedding decision](docs/decisions/0001-own-chromium-embedding.md), initial
[capability requirements](docs/contracts/engine-capabilities.md), and
[source-build preparation](engine/chromium/BUILDING.md) are recorded. They are
not runtime evidence. Record the native/runtime stack decision after the spike.

Work in this order:

1. Use the smallest end-to-end scenario: profile A remains under human control while a task page opens in profile B. Define exact targets, pending decisions, and observable failure states before writing the engine port.
2. Provision an external Chromium source workspace at the pinned revision. Synchronize dependencies/toolchains and validate the baseline. Record source/dependency state, patch set, GN arguments, SDK, platform, and architecture. A prebuilt Chromium browser is not a Content embedding SDK.
3. Implement the minimal Content bootstrap and native view host, starting on macOS arm64. Review each required component and default delegate; do not ship content-shell test defaults or depend on the full browser target. Preserve sandboxing and record startup/shutdown behavior.
4. Test native view embedding, resize, input focus, keyboard navigation, input methods, and accessibility exposure.
5. Wire two persistent browser contexts and their storage/services to core-selected profiles. Verify actual cookie and site-storage isolation and persistence after restart; do not infer isolation from object count.
6. Test background page creation and input without activating the human's window or changing the active tab.
7. Wire and test popup disposition, upload/download dialogs, permission requests, and renderer failure. Investigate PDF/printing as explicit component integrations, not automatic Content features.
8. Investigate credential and passkey integration separately. Content embedding is not evidence of Chrome password-manager parity or access to vendor services.
9. Compare native content-view hosting and offscreen composition only where needed. Record compositor, input, accessibility, and resource trade-offs.
10. Select the core language, native renderer, any Rust/C++ boundary, and candidate plugin isolation approach from this evidence.

A spike can begin on one development machine. The stage is not complete until the hard-boundary checks have run on Linux, macOS, and Windows. Linux display-server variants and CPU coverage must be recorded explicitly.

**Build capacity:** source acquisition and compilation need a deliberately
provisioned workspace and matching upstream dependencies/toolchains. A source
pin is not evidence of a successful build or working embedder. Do not replace
missing build evidence with mock engine claims or checkout-only test suites.

**Gate:** publish a capability matrix with tested, unsupported, and unverified states. Do not disable sandboxing to make the integration pass. If a hard requirement fails, revise the integration before writing the general-purpose DSL.

**Performance baseline:** record cold startup, idle CPU, process-tree memory, package/runtime disk footprint, and a fixed multi-page workload. Include hardware, build mode, page set, profile count, and measurement method. Do not invent target numbers before a baseline exists; choose budgets before polishing features.

### Stage 1: Establish a minimal usable browser core

**Question:** Can one simple browser run entirely through the intended public contracts?

Implement the [Chromium embedder](engine/chromium/README.md), the initial core contracts, and a temporary native UI.

Work packages:

- Stable identities for profiles, pages, windows, tasks, and operations. Native handles are adapter details, not public permanent IDs.
- Page creation, navigation, back/forward, reload, close, activation, and renderer-recovery events.
- Profile creation, durable storage ownership, restart recovery, and explicit deletion semantics.
- Permission decisions, popup handling, file upload, download state, cancellation, and errors.
- An OS-backed credential-provider boundary. Keep credential values out of general UI state and logs.
- A single operation dispatcher used by the reference UI and later agent/plugin callers.
- A minimal trusted recovery interface outside user customization.

Record the initial contracts in `docs/contracts/pages.md`, `profiles.md`, `operations.md`, and `permissions.md`.

Specify operation semantics before implementation. For example, a stale page ID must produce a defined error rather than act on a replacement page. Repeated close requests must have an explicit outcome. A rejected permission request must not leave a half-created privileged operation.

Use controlled fixtures for form state, separate account cookies, downloads, popups, and renderer errors. Use Chromium's storage facilities behind the embedder; do not attempt to recreate browser cookie security in a general application database.

**Gate:** the minimal UI can browse through public operations, profile data survives restart, and cross-profile leakage tests fail when isolation is deliberately broken. Verify required close-confirmation handling and that an isolated renderer failure leaves unrelated profile data intact.

**Not yet required:** a polished sidebar, a plugin marketplace, video replay, or a production password manager. Record any missing everyday browsing capabilities rather than describing the result as complete.

### Stage 2: Prove interface and behavior replacement

**Question:** Does the architecture permit a different browser, or only a different skin?

Implement the smallest useful UI language and one permissioned service extension. Develop these together so the DSL does not become a fixed menu of official features.

#### UI track

1. Define typed bindings to observable core state, local presentation state, and commands.
2. Support layout composition, collections, selection, conditions, and event bindings.
3. Make asynchronous loading, errors, invalidated references, and denied actions expressible.
4. Provide semantic controls and keyboard/accessibility behavior rather than only drawing primitives.
5. Build `presets/workspace/` with a sidebar, folders, and persistent versus temporary page organization.
6. Build `presets/classic/` with horizontal tabs and a conventional toolbar.
7. Run both against the same core and the same browser contract suite.

Folders and workspace membership should refer to core page identities without forcing one UI organization model into the engine adapter. Define what happens to those references when a page closes or a session restores.

#### Behavior track

Use account routing as the first service plugin. Supply two implementations: explicit user selection and a user-configured rule-based choice. Both must operate within granted profile access.

Define service registration, scope, precedence, timeout, cancellation, and fallback. Begin with one selected provider per service scope where that is sufficient. Do not invent a complex plugin-composition system before a real scenario needs it.

The host validates the result of a routing plugin before creating the page. A plugin cannot return an unauthorized profile and acquire access merely because its return value has the correct type.

Test crashes, invalid results, revoked grants, recursion, slow responses, and resource exhaustion. Select an isolation mechanism that can enforce the intended failure boundary. Arbitrary unrestricted native-code loading is not an acceptable substitute.

**Gate:** a new personal package changes both UI and routing behavior without editing core code. The official package has no hidden interface unavailable to the alternative. If a core change is necessary, explain which missing general capability it introduces.

### Stage 3: Integrate the workspace and agent experience

**Question:** Can one useful reference browser demonstrate both personalization and undisturbed human-agent work?

Polish the workspace preset first. Keep the classic preset as a compatibility and architectural-independence test.

Implement:

- Workspace navigation, persistent/temporary page handling, command navigation, and clear profile context.
- Explicit task creation with profile and operation grants.
- Task-page association, progress, errors, and an action history.
- Agent operations that use core capabilities without driving the customizable human UI.
- Pause, cancellation, inspection, and handoff to the human.
- Per-page and per-task cleanup with exact ownership tracking.

Start with a deterministic test client. Integrate an existing coding agent once the contract works; do not use model success as the only correctness test. Choose the agent transport after the operation contract, not before it.

The primary acceptance scenario is:

1. A human edits a form in profile A.
2. A granted agent opens and interacts with a controlled fixture in profile B.
3. Profile A's selected tab, foreground window, form state, and input focus remain unchanged by the agent.
4. The user revokes access or takes over the task.
5. Future unauthorized operations are blocked; pending operations report their actual status.
6. Task cleanup closes only pages it is authorized to close.

Permission and credential dialogs may require human interaction. Route these into an explicit pending/handoff state rather than stealing focus automatically.

Separate tabs do not isolate a remote cart or draft. Define coordination for shared external objects where possible and require confirmation for sensitive writes. Do not promise exactly-once remote effects or rollback of actions already accepted by a website.

**Gate:** execute the scenario on all supported desktop platforms. Record active page/window identities and actual focus behavior, not only successful protocol responses. A webpage cannot grant capabilities through instructions embedded in its content.

**Deferred:** full session video recording and replay. Reliable action history, page attribution, and task lifecycle come first.

### Stage 4: Deliver the local customization development loop

**Question:** Can a user's own coding agent build and debug a customization without a maintainer's help?

Connect the existing compiler, tests, runtime, and preview into a small toolchain under `tools/`.

Required operations, with command syntax still to be designed:

| Operation | Required result |
| --- | --- |
| Inspect capabilities | Installed contract versions, platform capabilities, signatures, and permission requirements. |
| Validate package | Syntax/type errors, invalid references, missing capabilities, and dependency conflicts. |
| Run tests | Machine-readable failures, scenario IDs, reproduction inputs, and evidence locations. |
| Preview | An isolated browser instance or environment using disposable data by default. |
| Inspect preview | Screenshot, semantic UI structure, logs, and current runtime errors. |
| Activate | Explicit approval of the exact package revision and requested grants. |
| Recover | Restore a known-good UI or disable a faulty plugin without losing the recovery interface. |

Use structured output and conventional exit statuses, while keeping readable messages for humans. Derive documentation and signatures from the same contract source when practical.

A package manifest should identify its format version, entry points, required capabilities, dependencies, local data schema, and requested permissions. These are proposed fields, not a frozen manifest format.

Do not give every preview real account access. Grant access explicitly for tests that require it. A preview should not accidentally execute a live payment or send a message.

**Gate:** two independent coding agents complete a UI change and a behavior-plugin change using the public documentation and toolchain. Review the resulting diffs and run the same platform tests. A human can use the complete loop without AI.

Do not require a hosted generation service, embedded model, or private agent-only API.

### Stage 5: Prove upgrades and recovery

**Question:** Can the foundation improve without turning every user into a fork maintainer?

Define `docs/contracts/compatibility.md` and package/data migration rules before broad distribution. Compatibility design starts in earlier stages; this stage exercises it end to end.

Version these independently:

- The UI language and package format.
- Core operation and service contracts.
- Plugin protocol or ABI, depending on the selected runtime.
- Personal-package data schemas.
- Engine and platform data formats owned by their respective adapters.

Build old-package fixtures and retain the original bytes in `tests/upgrade/`. Then test:

1. Upgrade Pliant with a compatible old UI and plugin. Neither requires an AI call.
2. Apply a deterministic supported migration and verify data preservation.
3. Build the embedder against two real Chromium revisions. Verify profile data and operation semantics, not only compilation; record upstream API adaptations and patch changes.
4. Load an unsupported UI package. Preserve it and offer the trusted default interface.
5. Load an incompatible credential or routing plugin. Stop affected operations; do not silently choose a different account or security policy.
6. Interrupt an upgrade and verify the documented recovery procedure.
7. Apply a security update while an incompatible customization remains disabled.

Do not promise arbitrary downgrades of engine profile data. Chromium or storage migrations may make binary rollback unsafe. Define backups, restore boundaries, and migration journals where applicable; test recovery on copies before touching user data.

**Gate:** an old compatible customization works after an actual foundation upgrade with AI disabled. An incompatible one cannot block security updates or silently change privileged behavior.

### Stage 6: Release quality and lightweight operation

**Question:** Is the result suitable for people to rely on, beyond the architectural demo?

Complete the platform matrix for native packaging, sandboxing, signing where applicable, update verification, crash reporting, and supported display configurations. Document minimum OS versions and architectures from tested evidence.

Expand coverage for:

- Input methods, keyboard layouts, accessibility, display scaling, and multiple monitors.
- Sleep/resume, offline transitions, proxy failures, disk-full errors, and process termination.
- Authentication redirects, passkeys, file dialogs, PDFs, clipboard, and downloads.
- Media and codec availability, including any licensing or distribution limits.
- Multiple profiles, windows, tasks, and conflicting plugins.
- Recovery after UI errors and invalid customization packages.

Confirm that Chrome/Firefox extension installation is not exposed through an unintended product path. Audit native and plugin dependencies separately.

Measure against Stage 0 with the same workload. Attribute memory to the whole process tree, including renderers and GPU processes. Distinguish private memory from shared mappings rather than summing incompatible metrics.

Reduce unnecessary caches, polling, background work, and duplicate infrastructure. Make page unloading an explicit policy with defined restoration behavior. Keep process isolation and permission enforcement intact.

**Gate:** publish a supported-capability matrix, reproducible performance results, known limitations, and a tested update/recovery path. Do not call the browser release-ready based only on a successful build.

## 4. Continuous verification

Testing and safety begin at Stage 0. They are not a final hardening phase.

For each capability:

1. Write its observable contract and failure behavior.
2. Add a regression test that fails without the capability or fix.
3. Implement the smallest supporting mechanism.
4. Run unit and contract tests.
5. Exercise the real browser/native path when the claim depends on it.
6. Record evidence and limitations before advancing the stage.

Use the following layers:

| Layer | What it establishes |
| --- | --- |
| Static validation | Known type, schema, dependency, and capability mismatches are rejected. |
| Unit and model tests | State transitions and local policy behavior match the contract. |
| Property and fault tests | Defined invariants survive generated inputs, interrupted operations, and failures. |
| Browser integration | Real Chromium embedder behavior, profiles, permissions, and native-window handling match the intended operation. |
| Preset/plugin conformance | The same guarantees apply to different interfaces and service implementations. |
| Upgrade tests | Old packages and data receive the documented compatibility or recovery behavior. |
| Human usability review | The interface is understandable and practical, beyond mechanically passing tests. |

Include deliberate negative fixtures: unauthorized profile selection, stale page references, malformed packages, exhausted plugin budgets, and unavailable capabilities. Tests must demonstrate that relevant violations are detected, not only that happy paths succeed.

A large suite is evidence, not proof that arbitrary local code is safe. Runtime controls enforce the boundary even for paths that tests did not anticipate.

## 5. Workstreams and review gates

After the feasibility decisions, separate work into engine/core, UI/DSL, plugin runtime, and verification/toolchain workstreams. Share versioned contracts before parallel implementation.

Do not let independent workers invent incompatible state models. Changes to an identity, operation, permission, or lifecycle contract must update its consumers and tests together.

Review each milestone for:

**Requirement fit:** can users replace the intended behavior, or did the implementation hard-code the official experience?

**Boundary safety:** are privileges enforced by the host, rather than assumed from a cooperative plugin?

**Evidence quality:** did the test exercise the real failure path, platform, and old-version package that the claim depends on?

Implementation, test design, and independent review should have different owners where practical. Generated code receives the same review standards as manually written code.

## 6. Open decisions and stop conditions

Resolve these before the dependent work proceeds:

| Decision | Evidence needed |
| --- | --- |
| Native UI framework | Content-view embedding, full layout replacement, accessibility, input methods, platform reach, and measured overhead. |
| UI language design | Both reference presets can be expressed without internal patches or unrestricted host-code escape hatches. |
| Plugin runtime | Enforceable grants, cancellation/resource limits, failure isolation, portability, and acceptable startup cost. |
| Data and identity model | Persistent multi-profile behavior, engine storage constraints, and recoverable local customization state. |
| Compatibility policy | Supported version windows, deterministic migrations, and explicit treatment of unavailable privileged services. |
| Distribution and licensing | Chromium/component dependency obligations, project license, source-build capacity, signing/update ownership, and tested release targets. |

Stop and revise the architecture if independent customization requires patching core internals, if profile isolation cannot be demonstrated, or if a failed plugin can bypass the recovery path.

A platform API gap is not permission to disable a security boundary. A missing test machine is not evidence of cross-platform support. Owning the embedder also means sustaining timely Chromium security updates; revisit the approach if that becomes unmaintainable.

## 7. What not to build yet

Defer mobile, multiple engine backends, a public package marketplace, built-in model hosting, and full video replay. Do not implement Chrome/Firefox extension compatibility later by default; changing that non-goal requires a separate product decision.

Do not attempt to copy all Arc or Neo features. One polished reference browser, a structurally different second preset, and a small set of meaningful service plugins are enough to test the thesis.

## Completion criteria for the first public prototype

- [ ] A Pliant-owned Chromium Content embedder operates through Pliant contracts on Linux, macOS, and Windows.
- [ ] Two distinct UI presets work without private core access.
- [ ] A user can replace one meaningful behavior service through the plugin contract.
- [ ] A human-agent background scenario passes with verified profile and focus behavior.
- [ ] A local coding agent can use the public write/test/preview/revise workflow.
- [ ] Runtime guards reject unauthorized actions and bound plugin failures.
- [ ] A compatible old package survives a real foundation upgrade without AI.
- [ ] Recovery preserves user data and does not silently replace privileged policies.
- [ ] Performance measurements and unsupported capabilities are documented.

All checkboxes are intentionally unchecked. They describe evidence the implementation must produce.
