let rules = import "rules.ncl" in
let bodies = import "modules.ncl" in
let sample = ["firefox", "zen-browser", "kanata", "users", "lane-gc", "atuin", "chrome", "godot", "bun", "obsidian"] in
let inputs = rules.flake_inputs [
  {name = "nur", url = "github:nix-community/NUR"},
  {name = "zen-browser", url = "github:0xc000022070/zen-browser-flake"},
] in
let export = fun host =>
  let resolved = rules.enabled host in
  let overrides = rules.tag_override host in
  let selected = std.array.filter (fun name => std.array.elem name sample) resolved in
  let host_settings = std.record.from_array (std.array.map (fun key => {field = key, value = {}}) selected) & host.settings in
  let raw = fun name =>
    (if std.record.has_field name host.settings then host.settings."%{name}" else {})
    & {enable = std.array.elem name resolved}
    & (if std.record.has_field name overrides then overrides."%{name}" else {}) in
  let declared = std.array.filter (rules.platform_gate host) sample in
  let facts = if std.record.has_field "responsibilities" host then host.responsibilities else [] in
  let required = std.array.map (std.array.at 2) (std.array.filter (fun triple => std.array.at 1 triple == "requires_software") facts) in
  let realizers = std.record.from_array (std.array.map (fun triple => {
    field = std.array.at 0 triple, value = [std.array.at 2 triple],
  }) (std.array.filter (fun triple => std.array.at 1 triple == "is_realized_by") facts)) in
  let realized = rules.realization (std.array.map (fun name => "%{name}-module") resolved) required realizers in
  if !realized then std.fail_with "required software has no enabled realizing module" else
  std.deep_seq [inputs, bodies.users_options (raw "users")] {
    enabled = std.array.filter (fun name => std.array.elem name resolved) sample,
    settings.myConfig.modules = host_settings & std.record.map (fun _ value =>
      std.record.map (fun _ literal => {"$call" = {function = {"$ref" = ["lib", "mkDefault"]}, args = [literal]}}) value
    ) overrides,
    modules = std.record.from_array (std.array.map (fun key => {
      field = key,
      value = if std.array.elem key ["lane-gc", "atuin"] then bodies."%{key}" (raw key) (raw "users") else bodies."%{key}" (raw key),
    }) declared),
  } in
{
  hosts | not_exported = import "hosts.ncl",
  whiterabbit = export hosts.whiterabbit,
  ashashi = export hosts.ashashi,
}
