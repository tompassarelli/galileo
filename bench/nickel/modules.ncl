let types = import "../schema/nickel/types.ncl" in
let ref = fun path => {"$ref" = path} in
let call = fun path call_args => {"$call" = {function = ref path, args = call_args}} in
let option = fun value => call ["lib", "mkOption"] [value] in
let mk_enable = fun description => call ["lib", "mkEnableOption"] [description] in
let mime = fun desktop => std.record.from_array (std.array.map (fun key => {field = key, value = desktop}) [
  "text/html", "x-scheme-handler/http", "x-scheme-handler/https", "x-scheme-handler/about", "x-scheme-handler/unknown",
]) in
{
  firefox = fun raw =>
    let cfg = ({enable | default = false, default | default = true} & raw) | {
      enable | types."myConfig.modules.firefox.enable",
      default | types."myConfig.modules.firefox.default",
    } in
    std.deep_seq cfg {
      options.myConfig.modules.firefox = {
        enable = mk_enable "Enable Firefox browser",
        default = option {type = ref ["lib", "types", "bool"], default = true, description = "Set Firefox as the default browser via MIME types"},
      },
      imports = [if cfg.enable then {config = {
        programs.firefox.enable | types."programs.firefox.enable" = true,
        xdg.mime = if cfg.default then {defaultApplications | types."xdg.mime.defaultApplications" = mime "firefox.desktop"} else {},
      }} else {}],
    },
  "zen-browser" = fun raw =>
    let cfg = ({enable | default = false, default | default = false} & raw) | {
      enable | types."myConfig.modules.zen-browser.enable",
      default | types."myConfig.modules.zen-browser.default",
    } in
    let packages | types."services.kanata.package" = ref ["inputs", "zen-browser", "packages"] in
    let system | types."services.kanata.package" = ref ["pkgs", "stdenv", "hostPlatform", "system"] in
    std.deep_seq cfg {
      options.myConfig.modules."zen-browser" = {
        enable = mk_enable "Enable Zen Browser",
        default = option {type = ref ["lib", "types", "bool"], default = false, description = "Set Zen Browser as the default browser via MIME types"},
      },
      imports = [if cfg.enable then {config = {
        environment.systemPackages = [call ["builtins", "getAttr"] ["default", call ["builtins", "getAttr"] [system, packages]]],
        xdg.mime = if cfg.default then {defaultApplications | types."xdg.mime.defaultApplications" = mime "zen.desktop"} else {},
      }} else {}],
    },
  kanata = fun raw =>
    let cfg = ({enable | default = false, devices | default = [], port | default = null, extraArgs | default = []} & raw) | {
      enable | types."myConfig.modules.kanata.enable",
      devices | types."myConfig.modules.kanata.devices",
      configFile | types."myConfig.modules.kanata.configFile",
      port | types."myConfig.modules.kanata.port",
      extraArgs | types."myConfig.modules.kanata.extraArgs",
    } in
    let has_devices = cfg.devices != [] in
    std.deep_seq cfg {
      options.myConfig.modules.kanata = {
        enable = mk_enable "Kanata keyboard remapping",
        devices = option {type = call ["lib", "types", "listOf"] [ref ["lib", "types", "str"]], default = [], description = "Input device paths for kanata to capture. Find yours with: ls /dev/input/by-id/"},
        configFile = option {type = ref ["lib", "types", "path"], description = "Path to kanata .kbd config file"},
        port = option {type = call ["lib", "types", "nullOr"] [ref ["lib", "types", "port"]], default = null, description = "TCP port for kanata server (e.g. for glide integration)"},
        extraArgs = option {type = call ["lib", "types", "listOf"] [ref ["lib", "types", "str"]], default = [], description = "Extra command-line args for kanata (e.g. --debug, --log-layer-changes for diagnosing layer/spacebar seizures)"},
      },
      imports = [if cfg.enable then {config = {
        hardware.uinput.enable | types."hardware.uinput.enable" = true,
        services.udev.extraRules | types."services.udev.extraRules" = "KERNEL==\"uinput\", MODE=\"0660\", GROUP=\"uinput\", OPTIONS+=\"static_node=uinput\"\n\n",
        users.groups | types."users.groups" = {uinput = {}, kanata = {}},
        users.users | types."users.users" = {kanata = {isSystemUser = true, group = "kanata", extraGroups = ["input", "uinput"]}},
        services.kanata = {
          enable | types."services.kanata.enable" = true,
          package | types."services.kanata.package" = ref ["pkgs", "unstable", "kanata"],
        } & (if has_devices then {keyboards | types."services.kanata.keyboards" = {main = {
          devices = cfg.devices,
          extraDefCfg = "process-unmapped-keys yes",
          extraArgs = cfg.extraArgs,
          config = call ["builtins", "readFile"] [cfg.configFile],
        } & (if cfg.port != null then {port = cfg.port} else {})}} else {}),
        systemd.services | types."systemd.services" = {"kanata-main" =
          if has_devices then {serviceConfig = {DynamicUser = call ["lib", "mkForce"] [false], User = "kanata"}} else {},
        },
      }} else {}],
    },
  users_options = fun raw =>
    ({
      enable | default = false,
      username | default = "user",
      email | default = "",
      fullName | default = "",
      homeDir | default = "/home/%{username}",
      codeDir | default = "%{homeDir}/code",
    } & raw) | {
      enable | types."myConfig.modules.users.enable",
      username | types."myConfig.modules.users.username",
      email | types."myConfig.modules.users.email",
      fullName | types."myConfig.modules.users.fullName",
      homeDir | types."myConfig.modules.users.homeDir",
      codeDir | types."myConfig.modules.users.codeDir",
    },
  users = fun raw =>
    let cfg = users_options raw in
    std.deep_seq cfg {imports = [{
      options.myConfig.modules.users = {
        enable = mk_enable "Enable user configuration",
        username = option {type = ref ["lib", "types", "str"], default = "user", description = "Primary system username (instance binds the real one)"},
        email = option {type = ref ["lib", "types", "str"], default = "", description = "Primary git/commit email"},
        fullName = option {type = ref ["lib", "types", "str"], default = "", description = "Git author / display name (git user.name)"},
        homeDir = option {type = ref ["lib", "types", "str"], default = cfg.homeDir, description = "User home directory"},
        codeDir = option {type = ref ["lib", "types", "str"], default = cfg.codeDir, description = "Root of source checkouts (the ~/code convention)"},
      },
    } & (if cfg.enable then {config = {
      users.users | types."users.users" = {"%{cfg.username}" = {
        shell = ref ["pkgs", "bashInteractive"], isNormalUser = true,
        home = cfg.homeDir, extraGroups = ["wheel", "networkmanager", "plugdev"],
      }},
      security.sudo.extraConfig | types."security.sudo.extraConfig" = "Defaults timestamp_timeout=30\nDefaults timestamp_type=global\n\n",
      systemd.tmpfiles.rules | types."systemd.tmpfiles.rules" = std.array.map (fun directory =>
        "d %{cfg.homeDir}/%{directory} 0755 %{cfg.username} users -"
      ) ["Documents", "Pictures/Screenshots", "code", "src"],
    }} else {})]},
  "lane-gc" = fun raw user_raw =>
    let cfg = raw | {enable | types."myConfig.modules.lane-gc.enable"} in
    let user = users_options user_raw in
    let description = "lane-gc: hourly retire landed, clean, idle worktrees and branches in ~/code containers; report unlanded work" in
    std.deep_seq [cfg, user] ({options.myConfig.modules."lane-gc".enable = mk_enable description} &
      (if cfg.enable then {config."home-manager".users | types."home-manager.users" = {"%{user.username}" = {
        systemd.user.services."lane-gc" = {
          Unit = {Description = description, OnFailure = ["update-notify@%n.service"]},
          Service = {
            Type = "oneshot", Nice = 10, TimeoutStartSec = "20min",
            Environment = [call ["lib", "concatStrings"] [[
              "PATH=/run/wrappers/bin:/run/current-system/sw/bin:%{user.homeDir}/.local/bin:",
              call ["lib", "makeBinPath"] [std.array.map (fun name => ref ["pkgs", name]) ["bun", "git", "openssh", "gh", "coreutils"]],
            ]]],
            ExecStart = "%{user.homeDir}/.local/bin/lane-gc",
          },
        },
        systemd.user.timers."lane-gc" = {
          Unit.Description = description,
          Timer = {OnBootSec = "10min", OnUnitActiveSec = "1h"},
          Install.WantedBy = ["timers.target"],
        },
      }}} else {})),
}
