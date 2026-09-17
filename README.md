# Pliant

**Infrastructure for your Coding Agent to build your browser.**

[Design philosophy](DESIGN_PHILOSOPHY.md) · [Implementation plan](IMPLEMENTATION_PLAN.md) · [Module map](MODULES.md) · [Discuss an idea](https://github.com/yifanswe/pliant-browser/issues)

![Pliant: one foundation, your browser on every device. Concept illustration.](assets/hero.png)

Pliant proposes a browser-building platform where users and their AI can replace the **entire interface and browser behavior**, without forking the foundation.

We also plan to provide **one or more ready-to-use browsers**: convenient defaults and reference implementations built through the same public interfaces available to everyone.

The repository now includes a documentation-only
[module scaffold](MODULES.md). It defines intended ownership and dependency
boundaries without choosing the host language, native UI framework, UI DSL, or
plugin runtime.

Check the scaffold and local Markdown links with:

```sh
python3 tools/check_structure.py
python3 -m unittest discover -s tests -p "test_*.py"
```

These commands test repository structure only. They do not build, run, or test
a browser.

## Customization with safety guards

**No Chrome/Firefox extension compatibility by design.** Users customize Pliant through its own plugins and declarative layouts, written directly or with their local coding agent.

Make an Arc-style workspace, a traditional tab bar, or your own desktop layout. Replace account-routing policies, credential providers, and session workflows through plugin contracts. Personal choices should not need an upstream PR.

The core should enforce safety boundaries while users change their experience. Infrastructure upgrades should remain possible **with or without AI**, independent of personal customizations.

## What we plan to provide

| Component | Purpose |
| --- | --- |
| **Web-engine abstraction** | Stable browser capabilities over CEF (Chromium Embedded Framework). |
| **Profile and data management** | Isolated accounts, cookies, passwords, permissions, and persistent data. |
| **Plugin runtime** | Extensible services and behavior with permissions and failure isolation. |
| **Declarative UI engine** | A concise DSL for complete interfaces and platform-specific desktop layouts. |
| **Tests and developer tools** | Comprehensive validation, isolated previews, and diagnostics for users and their AI. |
| **Upgrades and recovery** | Versioned contracts, migrations, and safe recovery without relying on AI. |

## Help shape Pliant

The initial engine backend is **CEF (Chromium Embedded Framework)**, targeting **Linux, macOS, and Windows** with native UI, without Electron. Mobile is deferred. DSL syntax, native UI framework, and plugin execution remain open decisions.

Bring a browser experience you want to build. The useful question is **which capabilities should the platform guarantee, and which decisions should users control?**

Read the [design philosophy](DESIGN_PHILOSOPHY.md) for the architecture, safety boundaries, and proposed acceptance criteria.

---

Concept by [Yifan Li](https://github.com/yifanswe).
