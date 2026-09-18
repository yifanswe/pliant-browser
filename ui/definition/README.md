# UI definition

`pliant-ui-definition` parses a bounded JSON document into native-control
descriptions. The schema supports row and column containers, spacers, literal
labels, address fields, page lists, one content surface, and buttons bound only
to `navigate`, `back`, `forward`, `close`, or `new_page`.

`parse_definition` rejects duplicate object keys at every nesting level,
validates the schema, and returns an immutable typed tree.
`resolve_address_open` turns a host-submitted address into either navigation of
the current page or creation of a new page. Submitted addresses are limited to
`MAX_RUNTIME_ADDRESS_BYTES` (2048 UTF-8 bytes). Both configured navigation
targets and submitted addresses accept only `http`, `https`, and `about:blank`.

`DefinitionState` owns exact source text and parsed snapshots. `preview`
replaces the pending candidate without changing active state; `apply` and
`reject` require that candidate's state-scoped ID; `reset` activates the
compiled safe definition. Cloning a state assigns the clone a fresh ID
namespace and remints any pending candidate ID. The host remains responsible
for file I/O and for keeping trusted apply, reject, and recovery controls
outside the user-defined tree.

This crate does not render native UI, persist definitions, watch files, execute
scripts, intercept webpage links, or alter `window.open`, redirect, and popup
semantics. Native host integration and native end-to-end evidence are separate
work.
