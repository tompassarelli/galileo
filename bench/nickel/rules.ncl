let context = import "../context.json" in
let sample = {
  firefox = {tags = ["browsers"], optins = []},
  "zen-browser" = {tags = [], optins = ["browsers"]},
  kanata = {tags = [], optins = []},
  users = {tags = [], optins = []},
  "lane-gc" = {tags = [], optins = []},
  atuin = {tags = ["terminal"], optins = []},
  chrome = {tags = [], optins = ["browsers"]},
  godot = {tags = ["creative"], optins = []},
  bun = {tags = ["javascript"], optins = []},
  obsidian = {tags = ["notes"], optins = []},
} in
let modules = context & sample in
{
  platform_gate = fun host name => host.platform == "linux" || std.array.elem name host.allowed,
  tag_resolution = fun host =>
    std.record.fields modules
    |> std.array.filter (fun name =>
      platform_gate host name
      && !std.array.elem name host.disabled
      && !std.array.elem name host.off
      && std.array.any (fun tag =>
        (std.array.elem tag.name modules."%{name}".tags && !std.array.elem name tag.minus)
        || (std.array.elem tag.name modules."%{name}".optins && std.array.elem name tag.plus)
      ) host.tags),
  enabled = fun host => std.array.dedup (tag_resolution host @ host.on),
  tag_override = fun host =>
    if std.array.elem "firefox" (tag_resolution host)
      && std.array.any (fun tag => tag.name == "browsers" && !std.array.elem "firefox" tag.minus) host.tags
    then {firefox.default = true} else {},
  realization = fun enabled required realizers =>
    std.array.all (fun software =>
      std.record.has_field software realizers
      && std.array.any (fun name => std.array.elem name enabled) realizers."%{software}"
    ) required,
  flake_inputs = fun inputs =>
    if std.array.all (fun input =>
      std.array.all (fun other => input.name != other.name || input.url == other.url) inputs
    ) inputs then inputs else std.fail_with "conflicting flake input URLs",
}
