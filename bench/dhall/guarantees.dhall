let n = ./names.dhall
let j = ./json.dhall
let exports = ./main.dhall
let Module = { mapKey : n.Name, mapValue : { tags : List n.Tag, optins : List n.Tag } }
let firefoxModule = { mapKey = n.Name.firefox, mapValue = { tags = [ n.Tag.browsers ], optins = [] : List n.Tag } }
let zenModule = { mapKey = n.Name.`zen-browser`, mapValue = { tags = [] : List n.Tag, optins = [ n.Tag.browsers ] } }
let obsidianModule = { mapKey = n.Name.obsidian, mapValue = { tags = [ n.Tag.notes ], optins = [] : List n.Tag } }
let atuinModule = { mapKey = n.Name.atuin, mapValue = { tags = [ n.Tag.terminal ], optins = [] : List n.Tag } }
let chromeModule = { mapKey = n.Name.chrome, mapValue = { tags = [] : List n.Tag, optins = [ n.Tag.browsers ] } }
let godotModule = { mapKey = n.Name.godot, mapValue = { tags = [ n.Tag.creative ], optins = [] : List n.Tag } }
let bunModule = { mapKey = n.Name.bun, mapValue = { tags = [ n.Tag.javascript ], optins = [] : List n.Tag } }
let declarations : List Module = [ firefoxModule, zenModule, { mapKey = n.Name.kanata, mapValue = { tags = [] : List n.Tag, optins = [] : List n.Tag } }, { mapKey = n.Name.users, mapValue = { tags = [] : List n.Tag, optins = [] : List n.Tag } }, { mapKey = n.Name.`lane-gc`, mapValue = { tags = [] : List n.Tag, optins = [] : List n.Tag } }, atuinModule, chromeModule, godotModule, bunModule, obsidianModule ]
let darwin = n.none // { atuin = True, bash = True, bc = True, beagle = True, btop = True, curl = True, delta = True, direnv = True, dust = True, eza = True, fastfetch = True, fd = True, `forgejo-cli` = True, gh = True, ghostscript = True, ghostty = True, git = True, gitleaks = True, glow = True, imagemagick = True, jq = True, kitty = True, procs = True, promptfoo = True, ripgrep = True, starship = True, tealdeer = True, tree = True, unrar = True, unzip = True, vim = True, wget = True, yazi = True, zoxide = True }
let whiterabbit = { linux = True, tags = n.noTags // { terminal = True, `cli-tools` = True, desktop = True, theming = True, auth = True, development = True, javascript = True, python = True, database = True, rust = True, lisp = True, racket = True, `doom-emacs` = True, browsers = True, gaming = True, creative = True, media = True, communication = True, office = True, notes = True, writing = True, vpn = True }, optins = n.none // { river = True, claude = True, `machine-update` = True, `agent-runtime-update` = True, `update-notify` = True, `proton-log-watchdog` = True, `skill-review-queue` = True, `vast-reaper` = True, socat = True, procps = True, `alsa-lib` = True, chrome = True, qutebrowser = True, `zen-browser` = True }, excluded = n.none // { mako = True, beagle = True, postgresql = True, godot = True, `youtube-music` = True, libreoffice = True, `protonvpn-cli` = True }, disabled = n.none // { anytype = True, bun = True, cargo = True, clang = True, clippy = True, clojure = True, dbeaver = True, drracket = True, gcc = True, glide = True, mail = True, nodejs = True, `obs-studio` = True, promptfoo = True, racket = True, `rust-analyzer` = True, rustc = True, rustfmt = True, slack = True, spotify = True, wine = True, wowup = True, zoom = True }, direct = n.none // { users = True, kanata = True, `lane-gc` = True, claude = True } }
let ashashi = { linux = False, tags = n.noTags // { terminal = True, `cli-tools` = True, development = True }, optins = n.none, excluded = n.none, disabled = n.none // { `bench-shield` = True, containers = True, `rebuild-nopasswd` = True }, direct = n.none }
let any = \(xs : List Bool) -> List/fold Bool xs Bool (\(x : Bool) -> \(rest : Bool) -> x || rest) False
let selected = \(host : { linux : Bool, tags : n.Tags, optins : n.Flags, excluded : n.Flags, disabled : n.Flags, direct : n.Flags }) -> \(module : Module) ->
  let tagged = \(tags : List n.Tag) -> any (j.map n.Tag Bool (\(tag : n.Tag) -> merge host.tags tag) tags)
  in (tagged module.mapValue.tags || (merge host.optins module.mapKey && tagged module.mapValue.optins)) && (merge host.excluded module.mapKey == False) && (merge host.disabled module.mapKey == False)
let supported = \(host : { linux : Bool, tags : n.Tags, optins : n.Flags, excluded : n.Flags, disabled : n.Flags, direct : n.Flags }) -> \(name : n.Name) -> host.linux || merge darwin name
let Path = List Text
let Projection = Path -> Text
let tail = \(path : Path) -> List/fold { index : Natural, value : Text } (List/indexed Text path) Path (\(x : { index : Natural, value : Text }) -> \(rest : Path) -> if Natural/isZero x.index then rest else [ x.value ] # rest) ([] : Path)
let leaf = \(value : Text) -> \(path : Path) -> if Natural/isZero (List/length Text path) then value else ""
let algebra : { array : List Projection -> Projection, bool : Bool -> Projection, double : Double -> Projection, integer : Integer -> Projection, null : Projection, object : List { mapKey : Text, mapValue : Projection } -> Projection, string : Text -> Projection } =
  { array = \(values : List Projection) -> \(path : Path) -> List/fold Projection values Text (\(value : Projection) -> \(rest : Text) -> value path ++ rest) ""
  , bool = \(value : Bool) -> leaf (if value then "True" else "False")
  , double = \(value : Double) -> leaf "number"
  , integer = \(value : Integer) -> leaf "number"
  , null = leaf "null"
  , string = \(value : Text) -> leaf (Text/show value)
  , object = \(fields : List { mapKey : Text, mapValue : Projection }) -> \(path : Path) -> merge
      { None = "present"
      , Some = \(key : Text) -> List/fold { mapKey : Text, mapValue : Projection } fields Text (\(field : { mapKey : Text, mapValue : Projection }) -> \(rest : Text) -> Text/replace ("<" ++ field.mapKey ++ ">") "" (Text/replace ("<" ++ key ++ ">") (field.mapValue (tail path)) ("<" ++ field.mapKey ++ ">")) ++ rest) ""
      } (List/head Text path)
  }
let removeAllowed = \(allowed : Module -> Bool) -> \(enabled : Text) -> List/fold Module declarations (Text -> Text) (\(module : Module) -> \(rest : Text -> Text) -> \(value : Text) -> rest (if allowed module then Text/replace (Text/show (n.text module.mapKey)) "" value else value)) (\(value : Text) -> value) enabled
let whiterabbitValue = exports.whiterabbit Projection algebra
let ashashiValue = exports.ashashi Projection algebra
let I1_whiterabbit_enabled_tag = assert : removeAllowed (\(module : Module) -> selected whiterabbit module || merge whiterabbit.direct module.mapKey) (whiterabbitValue [ "enabled" ]) === ""
let I1_ashashi_enabled_tag = assert : removeAllowed (\(module : Module) -> selected ashashi module || merge ashashi.direct module.mapKey) (ashashiValue [ "enabled" ]) === ""
let I2_whiterabbit_supported_platform = assert : removeAllowed (\(module : Module) -> supported whiterabbit module.mapKey) (whiterabbitValue [ "enabled" ]) === ""
let I2_ashashi_supported_platform = assert : removeAllowed (\(module : Module) -> supported ashashi module.mapKey) (ashashiValue [ "enabled" ]) === ""
let whiterabbitOverride = whiterabbitValue [ "settings", "myConfig", "modules", "firefox", "default" ]
let ashashiOverride = ashashiValue [ "settings", "myConfig", "modules", "firefox", "default" ]
let I3_whiterabbit_tag_override = assert : whiterabbitOverride === (if whiterabbit.tags.browsers && selected whiterabbit firefoxModule && supported whiterabbit n.Name.firefox then whiterabbitOverride else "")
let I3_ashashi_tag_override = assert : ashashiOverride === (if ashashi.tags.browsers && selected ashashi firefoxModule && supported ashashi n.Name.firefox then ashashiOverride else "")
let assignedRealizers = [] : List n.Name
let I4_enabled_realization = assert : List/fold n.Name assignedRealizers Bool (\(name : n.Name) -> \(rest : Bool) -> False && rest) True === True
in exports
