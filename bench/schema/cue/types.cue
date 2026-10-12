package schema

#Ref: {"$ref": string | [...string]}
#Call: {"$call": _}
#Package: #Ref
#Path: string | #Ref | #Call

#Options: {
  "environment.systemPackages": [...#Package]
  "hardware.uinput.enable": bool
  "home-manager.users": {[string]: _}
  "home-manager.users.<name>.programs.atuin.enable": _
  "home-manager.users.<name>.programs.atuin.enableFishIntegration": _
  "home-manager.users.<name>.programs.atuin.settings.auto_sync": _
  "home-manager.users.<name>.programs.atuin.settings.search_mode": _
  "home-manager.users.<name>.programs.atuin.settings.sync_frequency": _
  "home-manager.users.<name>.systemd.user.services.lane-gc.Service.Environment": _
  "home-manager.users.<name>.systemd.user.services.lane-gc.Service.ExecStart": _
  "home-manager.users.<name>.systemd.user.services.lane-gc.Service.Nice": _
  "home-manager.users.<name>.systemd.user.services.lane-gc.Service.TimeoutStartSec": _
  "home-manager.users.<name>.systemd.user.services.lane-gc.Service.Type": _
  "home-manager.users.<name>.systemd.user.services.lane-gc.Unit.Description": _
  "home-manager.users.<name>.systemd.user.services.lane-gc.Unit.OnFailure": _
  "home-manager.users.<name>.systemd.user.timers.lane-gc.Install.WantedBy": _
  "home-manager.users.<name>.systemd.user.timers.lane-gc.Timer.OnBootSec": _
  "home-manager.users.<name>.systemd.user.timers.lane-gc.Timer.OnUnitActiveSec": _
  "home-manager.users.<name>.systemd.user.timers.lane-gc.Unit.Description": _
  "myConfig.modules.atuin.enable": bool
  "myConfig.modules.bun.enable": bool
  "myConfig.modules.chrome.default": bool
  "myConfig.modules.chrome.enable": bool
  "myConfig.modules.firefox.default": bool
  "myConfig.modules.firefox.enable": bool
  "myConfig.modules.godot.enable": bool
  "myConfig.modules.kanata.configFile": #Path
  "myConfig.modules.kanata.devices": [...string]
  "myConfig.modules.kanata.enable": bool
  "myConfig.modules.kanata.extraArgs": [...string]
  "myConfig.modules.kanata.port": null | (int & >=0 & <=65535)
  "myConfig.modules.lane-gc.enable": bool
  "myConfig.modules.obsidian.enable": bool
  "myConfig.modules.users.codeDir": string
  "myConfig.modules.users.email": string
  "myConfig.modules.users.enable": bool
  "myConfig.modules.users.fullName": string
  "myConfig.modules.users.homeDir": string
  "myConfig.modules.users.username": string
  "myConfig.modules.zen-browser.default": bool
  "myConfig.modules.zen-browser.enable": bool
  "programs.firefox.enable": bool
  "security.sudo.extraConfig": string
  "services.kanata.enable": bool
  "services.kanata.keyboards": {[string]: {...}}
  "services.kanata.keyboards.main.config": {...}
  "services.kanata.keyboards.main.devices": {...}
  "services.kanata.keyboards.main.extraArgs": {...}
  "services.kanata.keyboards.main.extraDefCfg": {...}
  "services.kanata.keyboards.main.port": {...}
  "services.kanata.package": #Package
  "services.udev.extraRules": string
  "systemd.services": {[string]: {...}}
  "systemd.services.kanata-main.serviceConfig.DynamicUser": {...}
  "systemd.services.kanata-main.serviceConfig.User": {...}
  "systemd.tmpfiles.rules": [...string]
  "users.groups": {[string]: {...}}
  "users.groups.kanata": {...}
  "users.groups.uinput": {...}
  "users.users": {[string]: {...}}
  "users.users.<name>.extraGroups": {...}
  "users.users.<name>.home": {...}
  "users.users.<name>.isNormalUser": {...}
  "users.users.<name>.shell": {...}
  "users.users.kanata.extraGroups": {...}
  "users.users.kanata.group": {...}
  "users.users.kanata.isSystemUser": {...}
  "xdg.mime.defaultApplications": {[string]: _}
  "xdg.mime.defaultApplications.\"text/html\"": _
  "xdg.mime.defaultApplications.\"x-scheme-handler/about\"": _
  "xdg.mime.defaultApplications.\"x-scheme-handler/http\"": _
  "xdg.mime.defaultApplications.\"x-scheme-handler/https\"": _
  "xdg.mime.defaultApplications.\"x-scheme-handler/unknown\"": _
}
