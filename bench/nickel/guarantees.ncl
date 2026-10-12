let hosts = import "hosts.ncl" in
let declarations = (import "../context.json") & {
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
let contributes = fun host tag name =>
  !std.array.elem name host.disabled && !std.array.elem name host.off
  && ((std.array.elem tag.name declarations."%{name}".tags && !std.array.elem name tag.minus)
    || (std.array.elem tag.name declarations."%{name}".optins && std.array.elem name tag.plus)) in
let Supported = fun host name => host.platform == "linux" || std.array.elem name host.allowed in
let I1_Enabled_tag = fun host => std.contract.from_validator (fun value =>
  if std.array.all (fun name => std.array.elem name host.on
    || std.array.any (fun tag => contributes host tag name) host.tags) value.enabled
  then 'Ok else 'Error {message = "I1 Enabled tag: enabled module lacks a selected contributing tag or direct enable"}) in
let I2_Supported_platform = fun host => std.contract.from_validator (fun value =>
  if std.array.all (Supported host) value.enabled then 'Ok
  else 'Error {message = "I2 Supported platform: enabled module lacks platform support"}) in
let I3_Tag_override = fun host => std.contract.from_validator (fun value =>
  if !std.record.has_field "firefox" value.settings.myConfig.modules
    || !std.record.has_field "default" value.settings.myConfig.modules.firefox
    || std.array.any (fun tag => tag.name == "browsers"
      && contributes host tag "firefox" && Supported host "firefox") host.tags
  then 'Ok else 'Error {message = "I3 Tag override: browsers does not contribute firefox"}) in
let I4_Enabled_realization = fun host =>
  let assignments = [] in
  std.contract.from_validator (fun value =>
    if std.array.all (fun software => std.array.elem software value.enabled) assignments then 'Ok
    else 'Error {message = "I4 Enabled realization: assigned responsibility lacks enabled realizer"}) in
let Host = fun host => std.contract.all_of [
  I1_Enabled_tag host, I2_Supported_platform host, I3_Tag_override host, I4_Enabled_realization host,
] in
{
  whiterabbit | Host hosts.whiterabbit,
  ashashi | Host hosts.ashashi,
  ..
}
