# Pliant project guidelines

## Meaningful tests only

- Do not add tests or custom validators just for directory layouts, documentation links, source pins, Git checkouts, or other development scaffolding unless explicitly requested.
- Add tests when they protect implemented browser behavior, authorization/isolation, lifecycle, native integration, data integrity, upgrades, or a concrete regression.
- Unit/model tests and synthetic browser fixtures are useful when they exercise real product logic or failure cases. Do not mistake tests of bookkeeping or mocks alone for browser implementation or integration evidence.
- Prioritize product implementation over new validation infrastructure. If prerequisites block implementation, report the blocker rather than substituting test-only work.