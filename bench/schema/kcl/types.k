schema Package:
    "$ref": str | [str]

schema Call:
    "$call": any

type Path = str | Package | Call

schema Option000:
    name: str = "environment.systemPackages"
    value: [Package]

schema Option001:
    name: str = "hardware.uinput.enable"
    value: bool

schema Option002:
    name: str = "home-manager.users"
    value: {str:any}

schema Option003:
    name: str = "home-manager.users.<name>.programs.atuin.enable"
    value?: any

schema Option004:
    name: str = "home-manager.users.<name>.programs.atuin.enableFishIntegration"
    value?: any

schema Option005:
    name: str = "home-manager.users.<name>.programs.atuin.settings.auto_sync"
    value?: any

schema Option006:
    name: str = "home-manager.users.<name>.programs.atuin.settings.search_mode"
    value?: any

schema Option007:
    name: str = "home-manager.users.<name>.programs.atuin.settings.sync_frequency"
    value?: any

schema Option008:
    name: str = "home-manager.users.<name>.systemd.user.services.lane-gc.Service.Environment"
    value?: any

schema Option009:
    name: str = "home-manager.users.<name>.systemd.user.services.lane-gc.Service.ExecStart"
    value?: any

schema Option010:
    name: str = "home-manager.users.<name>.systemd.user.services.lane-gc.Service.Nice"
    value?: any

schema Option011:
    name: str = "home-manager.users.<name>.systemd.user.services.lane-gc.Service.TimeoutStartSec"
    value?: any

schema Option012:
    name: str = "home-manager.users.<name>.systemd.user.services.lane-gc.Service.Type"
    value?: any

schema Option013:
    name: str = "home-manager.users.<name>.systemd.user.services.lane-gc.Unit.Description"
    value?: any

schema Option014:
    name: str = "home-manager.users.<name>.systemd.user.services.lane-gc.Unit.OnFailure"
    value?: any

schema Option015:
    name: str = "home-manager.users.<name>.systemd.user.timers.lane-gc.Install.WantedBy"
    value?: any

schema Option016:
    name: str = "home-manager.users.<name>.systemd.user.timers.lane-gc.Timer.OnBootSec"
    value?: any

schema Option017:
    name: str = "home-manager.users.<name>.systemd.user.timers.lane-gc.Timer.OnUnitActiveSec"
    value?: any

schema Option018:
    name: str = "home-manager.users.<name>.systemd.user.timers.lane-gc.Unit.Description"
    value?: any

schema Option019:
    name: str = "myConfig.modules.atuin.enable"
    value: bool

schema Option020:
    name: str = "myConfig.modules.bun.enable"
    value: bool

schema Option021:
    name: str = "myConfig.modules.chrome.default"
    value: bool

schema Option022:
    name: str = "myConfig.modules.chrome.enable"
    value: bool

schema Option023:
    name: str = "myConfig.modules.firefox.default"
    value: bool

schema Option024:
    name: str = "myConfig.modules.firefox.enable"
    value: bool

schema Option025:
    name: str = "myConfig.modules.godot.enable"
    value: bool

schema Option026:
    name: str = "myConfig.modules.kanata.configFile"
    value: Path

schema Option027:
    name: str = "myConfig.modules.kanata.devices"
    value: [str]

schema Option028:
    name: str = "myConfig.modules.kanata.enable"
    value: bool

schema Option029:
    name: str = "myConfig.modules.kanata.extraArgs"
    value: [str]

schema Option030:
    name: str = "myConfig.modules.kanata.port"
    value?: int
    check:
        value >= 0 if value != None
        value <= 65535 if value != None

schema Option031:
    name: str = "myConfig.modules.lane-gc.enable"
    value: bool

schema Option032:
    name: str = "myConfig.modules.obsidian.enable"
    value: bool

schema Option033:
    name: str = "myConfig.modules.users.codeDir"
    value: str

schema Option034:
    name: str = "myConfig.modules.users.email"
    value: str

schema Option035:
    name: str = "myConfig.modules.users.enable"
    value: bool

schema Option036:
    name: str = "myConfig.modules.users.fullName"
    value: str

schema Option037:
    name: str = "myConfig.modules.users.homeDir"
    value: str

schema Option038:
    name: str = "myConfig.modules.users.username"
    value: str

schema Option039:
    name: str = "myConfig.modules.zen-browser.default"
    value: bool

schema Option040:
    name: str = "myConfig.modules.zen-browser.enable"
    value: bool

schema Option041:
    name: str = "programs.firefox.enable"
    value: bool

schema Option042:
    name: str = "security.sudo.extraConfig"
    value: str

schema Option043:
    name: str = "services.kanata.enable"
    value: bool

schema Option044:
    name: str = "services.kanata.keyboards"
    value: {str:{str:any}}

schema Option045:
    name: str = "services.kanata.keyboards.main.config"
    value: {str:any}

schema Option046:
    name: str = "services.kanata.keyboards.main.devices"
    value: {str:any}

schema Option047:
    name: str = "services.kanata.keyboards.main.extraArgs"
    value: {str:any}

schema Option048:
    name: str = "services.kanata.keyboards.main.extraDefCfg"
    value: {str:any}

schema Option049:
    name: str = "services.kanata.keyboards.main.port"
    value: {str:any}

schema Option050:
    name: str = "services.kanata.package"
    value: Package

schema Option051:
    name: str = "services.udev.extraRules"
    value: str

schema Option052:
    name: str = "systemd.services"
    value: {str:{str:any}}

schema Option053:
    name: str = "systemd.services.kanata-main.serviceConfig.DynamicUser"
    value: {str:any}

schema Option054:
    name: str = "systemd.services.kanata-main.serviceConfig.User"
    value: {str:any}

schema Option055:
    name: str = "systemd.tmpfiles.rules"
    value: [str]

schema Option056:
    name: str = "users.groups"
    value: {str:{str:any}}

schema Option057:
    name: str = "users.groups.kanata"
    value: {str:any}

schema Option058:
    name: str = "users.groups.uinput"
    value: {str:any}

schema Option059:
    name: str = "users.users"
    value: {str:{str:any}}

schema Option060:
    name: str = "users.users.<name>.extraGroups"
    value: {str:any}

schema Option061:
    name: str = "users.users.<name>.home"
    value: {str:any}

schema Option062:
    name: str = "users.users.<name>.isNormalUser"
    value: {str:any}

schema Option063:
    name: str = "users.users.<name>.shell"
    value: {str:any}

schema Option064:
    name: str = "users.users.kanata.extraGroups"
    value: {str:any}

schema Option065:
    name: str = "users.users.kanata.group"
    value: {str:any}

schema Option066:
    name: str = "users.users.kanata.isSystemUser"
    value: {str:any}

schema Option067:
    name: str = "xdg.mime.defaultApplications"
    value: {str:any}

schema Option068:
    name: str = "xdg.mime.defaultApplications.\"text/html\""
    value?: any

schema Option069:
    name: str = "xdg.mime.defaultApplications.\"x-scheme-handler/about\""
    value?: any

schema Option070:
    name: str = "xdg.mime.defaultApplications.\"x-scheme-handler/http\""
    value?: any

schema Option071:
    name: str = "xdg.mime.defaultApplications.\"x-scheme-handler/https\""
    value?: any

schema Option072:
    name: str = "xdg.mime.defaultApplications.\"x-scheme-handler/unknown\""
    value?: any
