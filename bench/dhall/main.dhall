let s = ../schema/dhall/types.dhall
let t = s.types
let n = ./names.dhall
let j = ./json.dhall
let m = ./modules.dhall
let context = ./out/context.dhall
let private = ./out/private.dhall
let obj = j.object
let str = j.string
let Module = { mapKey : n.Name, mapValue : { tags : List n.Tag, optins : List n.Tag } }
let firefoxModule = { mapKey = n.Name.firefox, mapValue = { tags = [ n.Tag.browsers ], optins = [] : List n.Tag } }
let zenModule = { mapKey = n.Name.`zen-browser`, mapValue = { tags = [] : List n.Tag, optins = [ n.Tag.browsers ] } }
let obsidianModule = { mapKey = n.Name.obsidian, mapValue = { tags = [ n.Tag.notes ], optins = [] : List n.Tag } }
let atuinModule = { mapKey = n.Name.atuin, mapValue = { tags = [ n.Tag.terminal ], optins = [] : List n.Tag } }
let chromeModule = { mapKey = n.Name.chrome, mapValue = { tags = [] : List n.Tag, optins = [ n.Tag.browsers ] } }
let godotModule = { mapKey = n.Name.godot, mapValue = { tags = [ n.Tag.creative ], optins = [] : List n.Tag } }
let bunModule = { mapKey = n.Name.bun, mapValue = { tags = [ n.Tag.javascript ], optins = [] : List n.Tag } }
let sample : List Module =
  [ firefoxModule
  , zenModule
  , { mapKey = n.Name.kanata, mapValue = { tags = [] : List n.Tag, optins = [] : List n.Tag } }
  , { mapKey = n.Name.users, mapValue = { tags = [] : List n.Tag, optins = [] : List n.Tag } }
  , { mapKey = n.Name.`lane-gc`, mapValue = { tags = [] : List n.Tag, optins = [] : List n.Tag } }
  , atuinModule
  , chromeModule
  , godotModule
  , bunModule
  , obsidianModule
  ]
let darwin = n.none // { atuin = True, bash = True, bc = True, beagle = True, btop = True, curl = True, delta = True, direnv = True, dust = True, eza = True, fastfetch = True, fd = True, `forgejo-cli` = True, gh = True, ghostscript = True, ghostty = True, git = True, gitleaks = True, glow = True, imagemagick = True, jq = True, kitty = True, procs = True, promptfoo = True, ripgrep = True, starship = True, tealdeer = True, tree = True, unrar = True, unzip = True, vim = True, wget = True, yazi = True, zoxide = True }
let platform = < linux | darwin >
let Software = < obsidian >
let InputURL = < NUR | Zen >
let Workflow = < `knowledge-authoring-workflow` >
let Capability = < `knowledge-authoring` >
let responsibilities =
  { softwareUnit = Software.obsidian
  , module = n.Name.obsidian
  , requiredSoftware = { workflow = Workflow.`knowledge-authoring-workflow`, software = Software.obsidian }
  , providedCapability = { software = Software.obsidian, capability = Capability.`knowledge-authoring` }
  , realization = { software = Software.obsidian, module = n.Name.obsidian }
  }
let whiterabbit =
  { platform = platform.linux
  , tags = n.noTags // { terminal = True, `cli-tools` = True, desktop = True, theming = True, auth = True, development = True, javascript = True, python = True, database = True, rust = True, lisp = True, racket = True, `doom-emacs` = True, browsers = True, gaming = True, creative = True, media = True, communication = True, office = True, notes = True, writing = True, vpn = True }
  , optins = n.none // { river = True, claude = True, `machine-update` = True, `agent-runtime-update` = True, `update-notify` = True, `proton-log-watchdog` = True, `skill-review-queue` = True, `vast-reaper` = True, socat = True, procps = True, `alsa-lib` = True, chrome = True, qutebrowser = True, `zen-browser` = True }
  , excluded = n.none // { mako = True, beagle = True, postgresql = True, godot = True, `youtube-music` = True, libreoffice = True, `protonvpn-cli` = True }
  , disabled = n.none // { anytype = True, bun = True, cargo = True, clang = True, clippy = True, clojure = True, dbeaver = True, drracket = True, gcc = True, glide = True, mail = True, nodejs = True, `obs-studio` = True, promptfoo = True, racket = True, `rust-analyzer` = True, rustc = True, rustfmt = True, slack = True, spotify = True, wine = True, wowup = True, zoom = True }
  , direct = n.none // { users = True, kanata = True, `lane-gc` = True, claude = True }
  , required = [ responsibilities.requiredSoftware.software ]
  , user = { username = "tom", email = private.whiterabbit.users.email, fullName = "tompassarelli" }
  }
let ashashi =
  { platform = platform.darwin
  , tags = n.noTags // { terminal = True, `cli-tools` = True, development = True }
  , optins = n.none
  , excluded = n.none
  , disabled = n.none // { `bench-shield` = True, containers = True, `rebuild-nopasswd` = True }
  , direct = n.none
  , required = [] : List Software
  , user = { username = "ashashi", email = private.ashashi.users.email, fullName = "tompassarelli" }
  }
let Host = { platform : platform, tags : n.Tags, optins : n.Flags, excluded : n.Flags, disabled : n.Flags, direct : n.Flags, required : List Software, user : { username : t.`myConfig.modules.users.username`, email : t.`myConfig.modules.users.email`, fullName : t.`myConfig.modules.users.fullName` } }
let any = \(xs : List Bool) -> List/fold Bool xs Bool (\(x : Bool) -> \(rest : Bool) -> x || rest) False
let accepts = \(host : Host) -> \(module : Module) ->
  let named = \(flags : n.Flags) -> merge flags module.mapKey
  let tagged = \(tags : List n.Tag) -> any (j.map n.Tag Bool (\(tag : n.Tag) -> merge host.tags tag) tags)
  let selected = tagged module.mapValue.tags || (named host.optins && tagged module.mapValue.optins) || named host.direct
  let supported = merge { linux = True, darwin = merge darwin module.mapKey } host.platform
  in selected && (if named host.excluded || named host.disabled then False else True) && supported
let select = \(host : Host) -> \(modules : List Module) -> List/fold Module modules (List n.Name) (\(module : Module) -> \(rest : List n.Name) -> if accepts host module then [ module.mapKey ] # rest else rest) ([] : List n.Name)
let inputs : { nur : InputURL, `zen-browser` : InputURL } = { nur = InputURL.NUR, `zen-browser` = InputURL.Zen }
let export = \(host : Host) ->
  let realized = List/fold Software host.required Bool (\(software : Software) -> \(rest : Bool) ->
    merge { obsidian =
      merge { obsidian = True } responsibilities.softwareUnit
      && merge (n.none // { obsidian = True }) responsibilities.module
      && merge { `knowledge-authoring-workflow` = True } responsibilities.requiredSoftware.workflow
      && merge { obsidian = merge { `knowledge-authoring` = True } responsibilities.providedCapability.capability } responsibilities.providedCapability.software
      && merge { obsidian = accepts host (obsidianModule // { mapKey = responsibilities.realization.module }) } responsibilities.realization.software
    } software && rest) True
  let urls = \(url : InputURL) -> merge { NUR = "github:nix-community/NUR", Zen = "github:0xc000022070/zen-browser-flake" } url
  let inputURLs = { nur = urls inputs.nur, `zen-browser` = urls inputs.`zen-browser` }
  let tagOverride : t.`myConfig.modules.firefox.default` = host.tags.browsers
  let ff : m.Firefox = { enable = accepts host firefoxModule, default = True }
  let zen : m.Zen = { enable = accepts host zenModule, default = False }
  let configFile : t.`myConfig.modules.kanata.configFile` = < String : Text | Ref : t.`services.kanata.package` | Call : { `$call` : j.JSON } >.Call { `$call` = obj (toMap { function = j.ref [ "lib", "concatStrings" ], args = j.array [ j.array [ j.ref [ "flakeRoot" ], str "/dotfiles/kanata/kanata.kbd" ] ] }) }
  let port : t.`myConfig.modules.kanata.port` = Some 7070
  let portBound = assert : s.validPort port === True
  let kanata : m.Kanata = { enable = merge host.direct n.Name.kanata, configFile = configFile, port = port, devices = private.whiterabbit.kanata.devices, extraArgs = [ "--log-layer-changes" ] }
  let user : m.Users = { enable = merge host.direct n.Name.users, username = host.user.username, email = host.user.email, fullName = host.user.fullName, homeDir = "/home/${host.user.username}", codeDir = "/home/${host.user.username}/code" }
  let lane : m.Lane = { enable = merge host.direct n.Name.`lane-gc`, username = user.username, homeDir = user.homeDir }
  let atuin : m.Atuin = { enable = accepts host atuinModule , username = user.username }
  let chrome : m.Chrome = { enable = accepts host chromeModule, default = False }
  let godot : m.Godot = { enable = accepts host godotModule }
  let bun : m.Bun = { enable = accepts host bunModule }
  let obsidian : m.Obsidian = { enable = accepts host obsidianModule }
  let bodies = List/fold { name : n.Name, body : j.JSON }
    [ { name = n.Name.firefox, body = m.firefox ff }, { name = n.Name.`zen-browser`, body = m.zen zen }, { name = n.Name.kanata, body = m.kanata kanata }, { name = n.Name.users, body = m.users user }, { name = n.Name.`lane-gc`, body = m.lane lane }, { name = n.Name.atuin, body = m.atuin atuin }, { name = n.Name.chrome, body = m.chrome chrome }, { name = n.Name.godot, body = m.godot godot }, { name = n.Name.bun, body = m.bun bun }, { name = n.Name.obsidian, body = m.obsidian obsidian } ]
    (List { mapKey : Text, mapValue : j.JSON }) (\(body : { name : n.Name, body : j.JSON }) -> \(rest : List { mapKey : Text, mapValue : j.JSON }) -> if merge { linux = True, darwin = merge darwin body.name } host.platform then [ { mapKey = n.text body.name, mapValue = body.body } ] # rest else rest) ([] : List { mapKey : Text, mapValue : j.JSON })
  let selected = select host (sample # context)
  let enabled = List/fold n.Name selected (List Text) (\(name : n.Name) -> \(rest : List Text) -> if merge (n.none // { firefox = True, `zen-browser` = True, kanata = True, users = True, `lane-gc` = True, atuin = True, chrome = True, godot = True, bun = True, obsidian = True }) name then [ n.text name ] # rest else rest) ([] : List Text)
  let settings = obj (toMap { myConfig = obj (toMap { modules = obj
    ( [ { mapKey = "users", mapValue = obj (toMap { username = str user.username, email = str user.email, fullName = str user.fullName }) } ]
      # (if ff.enable then [ { mapKey = "firefox", mapValue = if tagOverride then obj (toMap { default = j.call [ "lib", "mkDefault" ] [ j.boolean tagOverride ] }) else j.empty } ] else [] : List { mapKey : Text, mapValue : j.JSON })
      # (if kanata.enable then [ { mapKey = "kanata", mapValue = obj (toMap { configFile = j.path kanata.configFile, port = merge { None = j.null, Some = j.natural } kanata.port, devices = j.texts kanata.devices, extraArgs = j.texts kanata.extraArgs }) } ] else [] : List { mapKey : Text, mapValue : j.JSON })
      # List/fold Module sample (List { mapKey : Text, mapValue : j.JSON }) (\(module : Module) -> \(rest : List { mapKey : Text, mapValue : j.JSON }) -> if accepts host module && merge (n.none // { atuin = True, chrome = True, `zen-browser` = True, `lane-gc` = True, obsidian = True }) module.mapKey then [ { mapKey = n.text module.mapKey, mapValue = j.empty } ] # rest else rest) ([] : List { mapKey : Text, mapValue : j.JSON })
    ) }) })
  in { realized = realized, value = obj (toMap { enabled = j.texts enabled, inputs = obj (toMap { nur = str inputURLs.nur, `zen-browser` = str inputURLs.`zen-browser` }), modules = obj bodies, settings = settings }) }
let whiterabbitExport = export whiterabbit
let whiterabbitRealization = assert : whiterabbitExport.realized === True
let ashashiExport = export ashashi
let ashashiRealization = assert : ashashiExport.realized === True
in { whiterabbit = whiterabbitExport.value, ashashi = ashashiExport.value }
