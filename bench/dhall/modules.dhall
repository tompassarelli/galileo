let s = ../schema/dhall/types.dhall
let t = s.types
let j = ./json.dhall
let obj = j.object
let str = j.string
let bool = j.boolean
let pkg = \(parts : List Text) -> j.package ({ `$ref` = < Name : Text | Parts : List Text >.Parts parts } : t.`services.kanata.package`)
let mime = \(desktop : Text) -> obj (toMap { `text/html` = str desktop, `x-scheme-handler/http` = str desktop, `x-scheme-handler/https` = str desktop, `x-scheme-handler/about` = str desktop, `x-scheme-handler/unknown` = str desktop })
let boolOption = \(value : Bool) -> \(description : Text) -> j.option (obj (toMap { type = j.ref [ "lib", "types", "bool" ], default = bool value, description = str description }))
let strOption = \(value : Text) -> \(description : Text) -> j.option (obj (toMap { type = j.ref [ "lib", "types", "str" ], default = str value, description = str description }))
let Firefox = { enable : t.`myConfig.modules.firefox.enable`, default : t.`myConfig.modules.firefox.default` }
let firefox = \(cfg : Firefox) -> obj (toMap
  { options = obj (toMap { myConfig = obj (toMap { modules = obj (toMap { firefox = obj (toMap { enable = j.enable "Enable Firefox browser", default = boolOption True "Set Firefox as the default browser via MIME types" }) }) }) })
  , imports = j.array [ obj (toMap { config = if cfg.enable then obj (toMap { programs = obj (toMap { firefox = obj (toMap { enable = bool cfg.enable }) }), xdg = obj (toMap { mime = if cfg.default then obj (toMap { defaultApplications = mime "firefox.desktop" }) else j.empty }) }) else j.empty }) ]
  })
let Zen = { enable : t.`myConfig.modules.zen-browser.enable`, default : t.`myConfig.modules.zen-browser.default` }
let zen = \(cfg : Zen) ->
  obj (toMap
    { options = obj (toMap { myConfig = obj (toMap { modules = obj (toMap { `zen-browser` = obj (toMap { enable = j.enable "Enable Zen Browser", default = boolOption False "Set Zen Browser as the default browser via MIME types" }) }) }) })
    , imports = j.array [ obj (toMap { config = if cfg.enable then obj (toMap { environment = obj (toMap { systemPackages = j.array [ j.call [ "builtins", "getAttr" ] [ str "default", j.call [ "builtins", "getAttr" ] [ pkg [ "pkgs", "stdenv", "hostPlatform", "system" ], pkg [ "inputs", "zen-browser", "packages" ] ] ] ] }), xdg = obj (toMap { mime = if cfg.default then obj (toMap { defaultApplications = mime "zen.desktop" }) else j.empty }) }) else j.empty }) ]
    })
let Kanata = { enable : t.`myConfig.modules.kanata.enable`, configFile : t.`myConfig.modules.kanata.configFile`, devices : t.`myConfig.modules.kanata.devices`, port : t.`myConfig.modules.kanata.port`, extraArgs : t.`myConfig.modules.kanata.extraArgs` }
let kanata = \(cfg : Kanata) ->
  let listType = j.call [ "lib", "types", "listOf" ] [ j.ref [ "lib", "types", "str" ] ]
  let listOption = \(description : Text) -> j.option (obj (toMap { type = listType, default = j.array ([] : List j.JSON), description = str description }))
  let keyboard = toMap { devices = j.texts cfg.devices, extraArgs = j.texts cfg.extraArgs, extraDefCfg = str "process-unmapped-keys yes", config = j.call [ "builtins", "readFile" ] [ j.path cfg.configFile ] }
  let port = obj (merge { None = keyboard, Some = \(n : Natural) -> [ { mapKey = "port", mapValue = j.natural n } ] # keyboard } cfg.port)
  let hasDevices = (if Natural/isZero (List/length Text cfg.devices) then False else True)
  let uinput : t.`hardware.uinput.enable` = cfg.enable
  let serviceEnabled : t.`services.kanata.enable` = cfg.enable
  let udev : t.`services.udev.extraRules` = "KERNEL==\"uinput\", MODE=\"0660\", GROUP=\"uinput\", OPTIONS+=\"static_node=uinput\"\n\n"
  in obj (toMap
    { options = obj (toMap { myConfig = obj (toMap { modules = obj (toMap { kanata = obj (toMap
      { enable = j.enable "Kanata keyboard remapping"
      , devices = listOption "Input device paths for kanata to capture. Find yours with: ls /dev/input/by-id/"
      , configFile = j.option (obj (toMap { type = j.ref [ "lib", "types", "path" ], description = str "Path to kanata .kbd config file" }))
      , port = j.option (obj (toMap { type = j.call [ "lib", "types", "nullOr" ] [ j.ref [ "lib", "types", "port" ] ], default = j.null, description = str "TCP port for kanata server (e.g. for glide integration)" }))
      , extraArgs = listOption "Extra command-line args for kanata (e.g. --debug, --log-layer-changes for diagnosing layer/spacebar seizures)"
      }) }) }) })
    , imports = j.array [ obj (toMap { config = if cfg.enable then obj (toMap
      { hardware = obj (toMap { uinput = obj (toMap { enable = bool uinput }) })
      , services = obj (toMap { udev = obj (toMap { extraRules = str udev }), kanata = obj (toMap { enable = bool serviceEnabled, package = pkg [ "pkgs", "unstable", "kanata" ], keyboards = if hasDevices then obj (toMap { main = port }) else j.empty }) })
      , users = obj (toMap { groups = obj (toMap { kanata = j.empty, uinput = j.empty }), users = obj (toMap { kanata = obj (toMap { isSystemUser = bool True, group = str "kanata", extraGroups = j.texts [ "input", "uinput" ] }) }) })
      , systemd = obj (toMap { services = if hasDevices then obj (toMap { `kanata-main` = obj (toMap { serviceConfig = obj (toMap { DynamicUser = j.call [ "lib", "mkForce" ] [ bool False ], User = str "kanata" }) }) }) else j.empty })
      }) else j.empty }) ]
    })
let Users = { enable : t.`myConfig.modules.users.enable`, username : t.`myConfig.modules.users.username`, email : t.`myConfig.modules.users.email`, fullName : t.`myConfig.modules.users.fullName`, homeDir : t.`myConfig.modules.users.homeDir`, codeDir : t.`myConfig.modules.users.codeDir` }
let users = \(cfg : Users) ->
  let sudo : t.`security.sudo.extraConfig` = "Defaults timestamp_timeout=30\nDefaults timestamp_type=global\n\n"
  let rules : t.`systemd.tmpfiles.rules` = [ "d ${cfg.homeDir}/Documents 0755 ${cfg.username} users -", "d ${cfg.homeDir}/Pictures/Screenshots 0755 ${cfg.username} users -", "d ${cfg.homeDir}/code 0755 ${cfg.username} users -", "d ${cfg.homeDir}/src 0755 ${cfg.username} users -" ]
  in obj (toMap { imports = j.array [ obj (toMap
    { options = obj (toMap { myConfig = obj (toMap { modules = obj (toMap { users = obj (toMap
      { enable = j.enable "Enable user configuration"
      , username = strOption "user" "Primary system username (instance binds the real one)"
      , email = strOption "" "Primary git/commit email"
      , fullName = strOption "" "Git author / display name (git user.name)"
      , homeDir = strOption cfg.homeDir "User home directory"
      , codeDir = strOption cfg.codeDir "Root of source checkouts (the ~/code convention)"
      }) }) }) })
    , config = if cfg.enable then obj (toMap
      { users = obj (toMap { users = obj [ { mapKey = cfg.username, mapValue = obj (toMap { shell = pkg [ "pkgs", "bashInteractive" ], isNormalUser = bool True, home = str cfg.homeDir, extraGroups = j.texts [ "wheel", "networkmanager", "plugdev" ] }) } ] })
      , security = obj (toMap { sudo = obj (toMap { extraConfig = str sudo }) })
      , systemd = obj (toMap { tmpfiles = obj (toMap { rules = j.texts rules }) })
      }) else j.empty
    }) ] })
let Lane = { enable : t.`myConfig.modules.lane-gc.enable`, username : t.`myConfig.modules.users.username`, homeDir : t.`myConfig.modules.users.homeDir` }
let lane = \(cfg : Lane) ->
  let description = "lane-gc: hourly retire landed, clean, idle worktrees and branches in ~/code containers; report unlanded work"
  let environment : t.`home-manager.users.<name>.systemd.user.services.lane-gc.Service.Environment` = j.array [ j.call [ "lib", "concatStrings" ] [ j.array [ str "PATH=/run/wrappers/bin:/run/current-system/sw/bin:${cfg.homeDir}/.local/bin:", j.call [ "lib", "makeBinPath" ] [ j.array [ pkg [ "pkgs", "bun" ], pkg [ "pkgs", "git" ], pkg [ "pkgs", "openssh" ], pkg [ "pkgs", "gh" ], pkg [ "pkgs", "coreutils" ] ] ] ] ] ]
  let exec : t.`home-manager.users.<name>.systemd.user.services.lane-gc.Service.ExecStart` = str "${cfg.homeDir}/.local/bin/lane-gc"
  let nice : t.`home-manager.users.<name>.systemd.user.services.lane-gc.Service.Nice` = j.integer +10
  let timeout : t.`home-manager.users.<name>.systemd.user.services.lane-gc.Service.TimeoutStartSec` = str "20min"
  let serviceType : t.`home-manager.users.<name>.systemd.user.services.lane-gc.Service.Type` = str "oneshot"
  in obj (toMap
    { options = obj (toMap { myConfig = obj (toMap { modules = obj (toMap { `lane-gc` = obj (toMap { enable = j.enable description }) }) }) })
    , config = if cfg.enable then obj (toMap { `home-manager` = obj (toMap { users = obj [ { mapKey = cfg.username, mapValue = obj (toMap { systemd = obj (toMap { user = obj (toMap
      { services = obj (toMap { `lane-gc` = obj (toMap { Unit = obj (toMap { Description = str description, OnFailure = j.texts [ "update-notify@%n.service" ] }), Service = obj (toMap { Environment = environment, ExecStart = exec, Nice = nice, TimeoutStartSec = timeout, Type = serviceType }) }) })
      , timers = obj (toMap { `lane-gc` = obj (toMap { Unit = obj (toMap { Description = str description }), Timer = obj (toMap { OnBootSec = str "10min", OnUnitActiveSec = str "1h" }), Install = obj (toMap { WantedBy = j.texts [ "timers.target" ] }) }) })
      }) }) }) } ] }) }) else j.empty
    })
let Atuin = { enable : t.`myConfig.modules.atuin.enable`, username : t.`myConfig.modules.users.username` }
let atuin = \(cfg : Atuin) ->
  let enabled : t.`home-manager.users.<name>.programs.atuin.enable` = bool cfg.enable
  let fish : t.`home-manager.users.<name>.programs.atuin.enableFishIntegration` = bool True
  let autoSync : t.`home-manager.users.<name>.programs.atuin.settings.auto_sync` = bool True
  let search : t.`home-manager.users.<name>.programs.atuin.settings.search_mode` = str "fuzzy"
  let frequency : t.`home-manager.users.<name>.programs.atuin.settings.sync_frequency` = str "5m"
  in obj (toMap
    { options = obj (toMap { myConfig = obj (toMap { modules = obj (toMap { atuin = obj (toMap { enable = j.enable "atuin shell history sync" }) }) }) })
    , config = if cfg.enable then obj (toMap { `home-manager` = obj (toMap { users = obj [ { mapKey = cfg.username, mapValue = obj (toMap { programs = obj (toMap { atuin = obj (toMap { enable = enabled, enableFishIntegration = fish, settings = obj (toMap { auto_sync = autoSync, search_mode = search, sync_frequency = frequency }) }) }) }) } ] }) }) else j.empty
    })
let Chrome = { enable : t.`myConfig.modules.chrome.enable`, default : t.`myConfig.modules.chrome.default` }
let chrome = \(cfg : Chrome) -> obj (toMap
  { options = obj (toMap { myConfig = obj (toMap { modules = obj (toMap { chrome = obj (toMap { enable = j.enable "Enable Google Chrome browser", default = boolOption False "Set Chrome as the default browser via MIME types" }) }) }) })
  , config = if cfg.enable then obj (toMap { environment = obj (toMap { systemPackages = j.array [ pkg [ "pkgs", "google-chrome" ] ] }), xdg = obj (toMap { mime = if cfg.default then obj (toMap { defaultApplications = mime "google-chrome.desktop" }) else j.empty }) }) else j.empty
  })
let Godot = { enable : t.`myConfig.modules.godot.enable` }
let godot = \(cfg : Godot) -> obj
  (toMap { options = obj (toMap { myConfig = obj (toMap { modules = obj (toMap { godot = obj (toMap { enable = j.enable "Godot game engine" }) }) }) }) } # (if cfg.enable then toMap { config = obj (toMap { environment = obj (toMap { systemPackages = j.array [ pkg [ "pkgs", "unstable", "godot_4" ] ] }) }) } else [] : List { mapKey : Text, mapValue : j.JSON }))
let Bun = { enable : t.`myConfig.modules.bun.enable` }
let bun = \(cfg : Bun) -> obj
  (toMap { options = obj (toMap { myConfig = obj (toMap { modules = obj (toMap { bun = obj (toMap { enable = j.enable "Bun JavaScript runtime and package manager" }) }) }) }) } # (if cfg.enable then toMap { config = obj (toMap { environment = obj (toMap { systemPackages = j.array [ pkg [ "pkgs", "bun" ] ] }) }) } else [] : List { mapKey : Text, mapValue : j.JSON }))
let Obsidian = { enable : t.`myConfig.modules.obsidian.enable` }
let obsidian = \(cfg : Obsidian) -> obj (toMap
  { options = obj (toMap { myConfig = obj (toMap { modules = obj (toMap { obsidian = obj (toMap { enable = j.enable "Obsidian note-taking" }) }) }) })
  , config = if cfg.enable then obj (toMap { environment = obj (toMap { systemPackages = j.array [ pkg [ "pkgs", "obsidian" ] ] }) }) else j.empty
  })
in { Firefox = Firefox, Zen = Zen, Kanata = Kanata, Users = Users, Lane = Lane, Atuin = Atuin, Chrome = Chrome, Godot = Godot, Bun = Bun, Obsidian = Obsidian, firefox = firefox, zen = zen, kanata = kanata, users = users, lane = lane, atuin = atuin, chrome = chrome, godot = godot, bun = bun, obsidian = obsidian }
