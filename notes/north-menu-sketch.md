# examples/north-menu.gal in D-61's forms (sketch for box 8.8b)

DECISIONS D-61's target for `examples/north-menu.gal`, written before the
constructs exist, so no command has read it. It is a `.md` file because
every `.gal` under the project root is a module that the project's commands
read (SPEC §2). Box 8.8b copies the block into `examples/north-menu.gal`,
fixes what the built reader reports, and measures it.

Measured on the block below against `examples/north-menu.gal` at main
`1ee816a`:

| | Before | After |
| --- | --- | --- |
| Lines | 198 | 164 |
| Non-blank lines | 167 | 136 |
| `{` characters | 66 | 35 |
| `{` in declaration headers | 62 | 23 |
| `{value: T}` slots | 14 | 0 |
| `accept-menu` clauses | 3 (45 lines) | 1 (11 lines) |
| Lines over 120 characters | 11 | 7 |

Every behaviour is meant to be unchanged. The relation names the replay
compares change only by the field spelling: `North.menu-kind` for
`menu-kind`, `MenuRow.position` for `menu-row-position`, and so on, which
the replay's name map already does for `MenuRow.key`. The events, their slot
names and the `Menu` view's fields and types are unchanged, so 8.9's
generated bindings and north's adapter are too.

```text
type North
north-main is a North

state entity MenuRow
  key: Text
  label: Text
  description: Text
  annotation: Text
  order: Int

state for North
  menu-kind: Text
  menu-title: Text
  menu-query: Text
  menu-help: Text
  menu-empty: Text
  menu-selection: Int
  menu-row-count: Int
  host-effect: Text
  effect-payload: Text
  active-conversation: Text

derived for North
  menu-count: Int

derived for MenuRow
  visible: Bool
  position: Int

state known-menu-row: North lists MenuRow

given accepting a {kind: Text} menu requests {effect: Text}
  at most one effect per kind

effect menu-host-effect: North requests host effect {effect: Text} with payload {payload: Text}
  at most one per step
  settled by clear-host-effect

accepting a "work" menu requests "work-show"
accepting a "history" menu requests "switch-conversation"
accepting a "archived-history" menu requests "restore-conversation"
accepting a "models" menu requests "select-model"
accepting a "efforts" menu requests "select-effort"
accepting a "config" menu requests "toggle-config"

rule menu-row-visible: ?row.visible = contains(lowercase("{?row.label} {?row.description} {?row.key}"), lowercase(?north.menu-query))
  when ?north lists ?row

rule menu-row-position: ?row.position = ?rank
  when ?north lists ?row
  and ?rank = count where ?north lists ?prior and ?prior.visible and ?prior.order < ?row.order

rule menu-count: ?north.menu-count = ?visible
  when ?north is a North
  and ?visible = count where ?north lists ?row and ?row.visible

define menu-help(kind: Text): Text
  if kind == "document" then "↑/↓ scroll · Esc close · /work claim, release, need, unneed or close to edit"
  else if kind == "work" then "Type to filter · ↑/↓ choose · Enter open · Esc close · /work search TEXT includes closed issues"
  else "Type to search · ↑/↓ choose · Enter open · Esc close"

define work-annotation(state: Text, blocked: Bool, holder: Text): Text
  if state == "closed" then "closed"
  else if blocked then "blocked"
  else if holder == "" then "ready"
  else "held"

define wrap(index: Int, count: Int): Int
  if index < 0 then count - 1 else if index >= count then 0 else index

initially
  north-main.menu-kind = ""
  north-main.menu-title = ""
  north-main.menu-query = ""
  north-main.menu-help = menu-help("")
  north-main.menu-empty = "No matching conversations"
  north-main.menu-selection = 0
  north-main.menu-row-count = 0
  north-main.host-effect = ""
  north-main.effect-payload = ""
  north-main.active-conversation = ""

event clear-menu-rows North
event open-history-menu North {kind: Text}
event open-settings-menu North {kind: Text} {title: Text}
event offer-work-row North {reference: Text} {title: Text} {state: Text} {holder: Text} {clock: Text} {eta: Text} {blocked: Bool}
event offer-settings-row North {key: Text} {label: Text} {description: Text} {annotation: Text}
event offer-history-row North {key: Text} {label: Text} {description: Text}
event accept-menu North
event query-menu North {query: Text}
event move-menu North {delta: Int}
event close-menu North
event clear-host-effect North

on clear-menu-rows ?north
  when ?north lists ?row
  then not ?row is a MenuRow
  may skip

on open-history-menu ?north ?kind
  then ?north.menu-kind = ?kind
  and ?north.menu-title = if ?kind == "archived-history" then "Archived Conversations" else "Resume Conversation"
  and ?north.menu-row-count = 0

on open-settings-menu ?north ?kind ?title
  then ?north.menu-kind = ?kind
  and ?north.menu-title = ?title
  and ?north.menu-row-count = 0
  and ?north.menu-help = menu-help(?kind)
  and ?north.menu-empty = if ?kind == "work" then "No matching work items" else "No matching entries"

on offer-work-row ?north ?reference ?title ?state ?holder ?clock ?eta ?blocked
  when ?description = if ?holder == "" then "No holder" else "{?holder} · {?clock} held / {?eta} ETA"
  then ?north.menu-row-count = ?north.menu-row-count + 1
  and ?north lists ?row
  and ?row is a new MenuRow {key: ?reference, label: "{?reference}  {?title}", description: ?description, annotation: work-annotation(?state, ?blocked, ?holder), order: ?north.menu-row-count}
  may skip

on offer-settings-row ?north ?key ?label ?description ?annotation
  then ?north.menu-row-count = ?north.menu-row-count + 1
  and ?north lists ?row
  and ?row is a new MenuRow {key: ?key, label: ?label, description: ?description, annotation: ?annotation, order: ?north.menu-row-count}

on offer-history-row ?north ?key ?label ?description
  when ?annotation = if ?key == ?north.active-conversation then "current" else ""
  then ?north.menu-row-count = ?north.menu-row-count + 1
  and ?north lists ?row
  and ?row is a new MenuRow {key: ?key, label: ?label, description: ?description, annotation: ?annotation, order: ?north.menu-row-count}

on accept-menu ?north
  when accepting a ?north.menu-kind menu requests ?effect
  and ?north.host-effect == ""
  and ?north lists ?row
  and ?row.visible
  and ?row.position == ?north.menu-selection
  then ?north.host-effect = ?effect
  and ?north.effect-payload = ?row.key
  and ?north requests host effect ?effect with payload ?row.key
  and ?north.menu-kind = if ?north.menu-kind == "work" then "work" else ""
  may skip

on query-menu ?north ?query
  then ?north.menu-query = ?query
  and ?north.menu-selection = 0

on move-menu ?north ?delta
  when ?north.menu-count > 0
  then ?north.menu-selection = wrap(?north.menu-selection + ?delta, ?north.menu-count)

on close-menu ?north
  then ?north.menu-kind = ""

on clear-host-effect ?north
  then ?north.host-effect = ""
  and ?north.effect-payload = ""

view Menu for North
  kind: ?north.menu-kind
  title: ?north.menu-title
  query: ?north.menu-query
  help: ?north.menu-help
  empty: ?north.menu-empty
  selection: ?north.menu-selection
  rows: all ?entry where ?north lists ?row and ?row.visible and ?entry = {row: ?row, key: ?row.key, label: ?row.label, description: ?row.description, annotation: ?row.annotation, position: ?row.position}, sorted by ?entry.position
```
