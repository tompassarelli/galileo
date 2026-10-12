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
  let packages : t.`services.kanata.package` = { `$ref` = < Name : Text | Parts : List Text >.Parts [ "inputs", "zen-browser", "packages" ] }
  let system : t.`services.kanata.package` = { `$ref` = < Name : Text | Parts : List Text >.Parts [ "pkgs", "stdenv", "hostPlatform", "system" ] }
  in obj (toMap
    { options = obj (toMap { myConfig = obj (toMap { modules = obj (toMap { `zen-browser` = obj (toMap { enable = j.enable "Enable Zen Browser", default = boolOption False "Set Zen Browser as the default browser via MIME types" }) }) }) })
    , imports = j.array [ obj (toMap { config = if cfg.enable then obj (toMap { environment = obj (toMap { systemPackages = j.array [ j.call [ "builtins", "getAttr" ] [ str "default", j.call [ "builtins", "getAttr" ] [ j.package system, j.package packages ] ] ] }), xdg = obj (toMap { mime = if cfg.default then obj (toMap { defaultApplications = mime "zen.desktop" }) else j.empty }) }) else j.empty }) ]
    })
let Kanata = { enable : t.`myConfig.modules.kanata.enable`, configFile : t.`myConfig.modules.kanata.configFile`, devices : t.`myConfig.modules.kanata.devices`, port : t.`myConfig.modules.kanata.port`, extraArgs : t.`myConfig.modules.kanata.extraArgs` }
let kanata = \(cfg : Kanata) -> \(bounded : s.validPort cfg.port === True) ->
  let listType = j.call [ "lib", "types", "listOf" ] [ j.ref [ "lib", "types", "str" ] ]
  let listOption = \(description : Text) -> j.option (obj (toMap { type = listType, default = j.array ([] : List j.JSON), description = str description }))
  let keyboard = obj (toMap { devices = j.texts cfg.devices, extraArgs = j.texts cfg.extraArgs, extraDefCfg = str "process-unmapped-keys yes", config = j.call [ "builtins", "readFile" ] [ j.path cfg.configFile ] })
  let port = merge { None = keyboard, Some = \(n : Natural) -> obj ([ { mapKey = "port", mapValue = j.natural n } ] # toMap { devices = j.texts cfg.devices, extraArgs = j.texts cfg.extraArgs, extraDefCfg = str "process-unmapped-keys yes", config = j.call [ "builtins", "readFile" ] [ j.path cfg.configFile ] }) } cfg.port
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
in { Firefox = Firefox, Zen = Zen, Kanata = Kanata, Users = Users, Lane = Lane, firefox = firefox, zen = zen, kanata = kanata, users = users, lane = lane }
