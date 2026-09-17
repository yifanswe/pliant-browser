# Pliant

**Infrastructure for your AI to build your browser.**

[Design philosophy](DESIGN_PHILOSOPHY.md) · [Discuss an idea](https://github.com/yifanswe/pliant-browser/issues)

![Pliant: one foundation, your browser on every device. Concept illustration.](assets/hero.png)

Pliant proposes a browser-building platform where users and their AI can replace the **entire interface and browser behavior**, without forking the foundation.

We also plan to provide **one or more ready-to-use browsers**: convenient defaults and reference implementations built through the same public interfaces available to everyone.

> **Design proposal.** No browser binary or working SDK is available yet. Images illustrate the concept, not a shipped application.

## Customization with safety guards

Make an Arc-style workspace, a traditional tab bar, or a different mobile layout. Replace account-routing policies, credential providers, and session workflows through plugin contracts. Personal choices should not need an upstream PR.

The core should enforce safety boundaries while users change their experience. Infrastructure upgrades should remain possible **with or without AI**, independent of personal customizations.

## What we plan to provide

| Component | Purpose |
| --- | --- |
| **Web-engine abstraction** | Stable browser capabilities over an existing engine. |
| **Profile and data management** | Isolated accounts, cookies, passwords, permissions, and persistent data. |
| **Plugin runtime** | Extensible services and behavior with permissions and failure isolation. |
| **Declarative UI engine** | A concise DSL for complete interfaces, including desktop and mobile variants. |
| **Tests and developer tools** | Comprehensive validation, isolated previews, and diagnostics for users and their AI. |
| **Upgrades and recovery** | Versioned contracts, migrations, and safe recovery without relying on AI. |

## Help shape Pliant

The first implementation target is native macOS; cross-platform customization is a design goal. Engine integration, DSL syntax, and plugin execution remain open decisions.

Bring a browser experience you want to build. The useful question is **which capabilities should the platform guarantee, and which decisions should users control?**

Read the [design philosophy](DESIGN_PHILOSOPHY.md) for the architecture, safety boundaries, and proposed acceptance criteria.

---

Concept by [Yifan Li](https://github.com/yifanswe).

Drafted and sent by Yifan's personally built AI assistant.
