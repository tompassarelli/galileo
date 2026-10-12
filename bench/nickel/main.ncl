let rules = import "rules.ncl" in
let bodies = import "modules.ncl" in
let sample = ["firefox", "zen-browser", "kanata", "users", "lane-gc", "atuin", "chrome", "godot", "bun", "obsidian"] in
let part1 = ["firefox", "zen-browser", "kanata", "users", "lane-gc"] in
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
  let declared = if host.platform == "linux" then part1 else std.array.filter (fun name => std.array.elem name host.allowed) part1 in
  std.deep_seq [inputs, bodies.users_options (raw "users")] {
    enabled = std.array.filter (fun name => std.array.elem name resolved) part1,
    settings.myConfig.modules = host_settings & std.record.map (fun _ value =>
      std.record.map (fun _ literal => {"$call" = {function = {"$ref" = ["lib", "mkDefault"]}, args = [literal]}}) value
    ) overrides,
    modules = std.record.from_array (std.array.map (fun key => {
      field = key,
      value = if key == "lane-gc" then bodies."lane-gc" (raw key) (raw "users") else bodies."%{key}" (raw key),
    }) declared),
  } in
{
  hosts | not_exported = import "hosts.ncl",
  whiterabbit = export hosts.whiterabbit,
  ashashi = export hosts.ashashi,
}
