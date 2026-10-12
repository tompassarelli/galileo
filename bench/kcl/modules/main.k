_call = lambda path, args { {"$call" = {function = {"$ref" = path}, args = args}} }
_option = lambda value { _call(["lib", "mkOption"], [value]) }
_enable = lambda description { _call(["lib", "mkEnableOption"], [description]) }
_mime = ["text/html", "x-scheme-handler/http", "x-scheme-handler/https", "x-scheme-handler/about", "x-scheme-handler/unknown"]

schema Firefox:
    enable: Option024
    default: Option023
    body: {str:any} = {
        options.myConfig.modules.firefox = {
            enable = _enable("Enable Firefox browser")
            default = _option({type = {"$ref" = ["lib", "types", "bool"]}, default = True, description = "Set Firefox as the default browser via MIME types"})
        }
        imports = [{
            if enable.value:
                config = {
                    programs.firefox.enable = Option041 {value = True}.value
                    xdg.mime = {
                        if default.value:
                            defaultApplications = {mime: "firefox.desktop" for mime in _mime}
                    }
                }
        }]
    }

schema ZenBrowser:
    enable: Option040
    default: Option039
    _packages = Package {"$ref" = ["inputs", "zen-browser", "packages"]}
    _system = Package {"$ref" = ["pkgs", "stdenv", "hostPlatform", "system"]}
    body: {str:any} = {
        options.myConfig.modules = {
            "zen-browser" = {
                enable = _enable("Enable Zen Browser")
                default = _option({type = {"$ref" = ["lib", "types", "bool"]}, default = False, description = "Set Zen Browser as the default browser via MIME types"})
            }
        }
        imports = [{
            if enable.value:
                config = {
                    environment.systemPackages = [_call(["builtins", "getAttr"], ["default", _call(["builtins", "getAttr"], [_system, _packages])])]
                    xdg.mime = {
                        if default.value:
                            defaultApplications = {mime: "zen.desktop" for mime in _mime}
                    }
                }
        }]
    }

schema Kanata:
    enable: Option028
    configFile: Option026
    devices: Option027
    port: Option030
    extraArgs: Option029
    body: {str:any} = {
        options.myConfig.modules.kanata = {
            enable = _enable("Kanata keyboard remapping")
            devices = _option({type = _call(["lib", "types", "listOf"], [{"$ref" = ["lib", "types", "str"]}]), default = [], description = "Input device paths for kanata to capture. Find yours with: ls /dev/input/by-id/"})
            configFile = _option({type = {"$ref" = ["lib", "types", "path"]}, description = "Path to kanata .kbd config file"})
            port = _option({type = _call(["lib", "types", "nullOr"], [{"$ref" = ["lib", "types", "port"]}]), default = None, description = "TCP port for kanata server (e.g. for glide integration)"})
            extraArgs = _option({type = _call(["lib", "types", "listOf"], [{"$ref" = ["lib", "types", "str"]}]), default = [], description = "Extra command-line args for kanata (e.g. --debug, --log-layer-changes for diagnosing layer/spacebar seizures)"})
        }
        imports = [{
            if enable.value:
                config = {
                    hardware.uinput.enable = Option001 {value = True}.value
                    services.udev.extraRules = Option051 {value = 'KERNEL=="uinput", MODE="0660", GROUP="uinput", OPTIONS+="static_node=uinput"\n\n'}.value
                    users.groups = Option056 {
                        value = {uinput = {}, kanata = {}}
                    }.value
                    users.users = Option059 {
                        value = {
                            kanata = {isSystemUser = True, group = "kanata", extraGroups = ["input", "uinput"]}
                        }
                    }.value
                    services.kanata = {
                        enable = Option043 {value = True}.value
                        package = Option050 {
                            value = {"$ref" = ["pkgs", "unstable", "kanata"]}
                        }.value
                        keyboards = {
                            if len(devices.value) > 0:
                                main = {
                                    devices = devices.value
                                    if port.value != None:
                                        port = port.value
                                    extraDefCfg = "process-unmapped-keys yes"
                                    extraArgs = extraArgs.value
                                    config = _call(["builtins", "readFile"], [configFile.value])
                                }
                        }
                    }
                    systemd.services = {
                        if len(devices.value) > 0:
                            "kanata-main" = {
                                serviceConfig = {DynamicUser = _call(["lib", "mkForce"], [False]), User = "kanata"}
                            }
                    }
                }
        }]
    }

schema Users:
    enable: Option035
    username: Option038
    email: Option034
    fullName: Option036
    homeDir: Option037
    codeDir: Option033
    body: {str:any} = {
        imports = [{
            options.myConfig.modules.users = {
                enable = _enable("Enable user configuration")
                username = _option({type = {"$ref" = ["lib", "types", "str"]}, default = "user", description = "Primary system username (instance binds the real one)"})
                email = _option({type = {"$ref" = ["lib", "types", "str"]}, default = "", description = "Primary git/commit email"})
                fullName = _option({type = {"$ref" = ["lib", "types", "str"]}, default = "", description = "Git author / display name (git user.name)"})
                homeDir = _option({type = {"$ref" = ["lib", "types", "str"]}, default = homeDir.value, description = "User home directory"})
                codeDir = _option({type = {"$ref" = ["lib", "types", "str"]}, default = codeDir.value, description = "Root of source checkouts (the ~/code convention)"})
            }
            if enable.value:
                config = {
                    users.users = Option059 {
                        value = {
                            "${username.value}" = {shell = Package {"$ref" = ["pkgs", "bashInteractive"]}, isNormalUser = True, home = homeDir.value, extraGroups = ["wheel", "networkmanager", "plugdev"]}
                        }
                    }.value
                    security.sudo.extraConfig = Option042 {value = "Defaults timestamp_timeout=30\nDefaults timestamp_type=global\n\n"}.value
                    systemd.tmpfiles.rules = Option055 {value = ["d ${homeDir.value}/${directory} 0755 ${username.value} users -" for directory in ["Documents", "Pictures/Screenshots", "code", "src"]]}.value
                }
        }]
    }

schema LaneGC:
    enable: Option031
    username: Option038
    homeDir: Option037
    _description = "lane-gc: hourly retire landed, clean, idle worktrees and branches in ~/code containers; report unlanded work"
    _packages = Option000 {value = [Package {"$ref" = ["pkgs", package]} for package in ["bun", "git", "openssh", "gh", "coreutils"]]}
    body: {str:any} = {
        options.myConfig.modules = {
            "lane-gc" = {
                enable = _enable(_description)
            }
        }
        if enable.value:
            config = {
                "home-manager" = {users = Option002 {
                    value = {
                        "${username.value}" = {
                            systemd.user.services = {
                                "lane-gc" = {
                                    Unit = {Description = _description, OnFailure = ["update-notify@%n.service"]}
                                    Service = {
                                        Type = "oneshot"
                                        Nice = 10
                                        TimeoutStartSec = "20min"
                                        Environment = [_call(["lib", "concatStrings"], [["PATH=/run/wrappers/bin:/run/current-system/sw/bin:${homeDir.value}/.local/bin:", _call(["lib", "makeBinPath"], [_packages.value])]])]
                                        ExecStart = homeDir.value + "/.local/bin/lane-gc"
                                    }
                                }
                            }
                            systemd.user.timers = {
                                "lane-gc" = {Unit = {Description = _description}, Timer = {OnBootSec = "10min", OnUnitActiveSec = "1h"}, Install = {WantedBy = ["timers.target"]}}
                            }
                        }
                    }
                }.value}
            }
    }

schema Atuin:
    enable: Option019
    username: Option038
    body: {str:any} = {
        options.myConfig.modules.atuin.enable = _enable("atuin shell history sync")
        if enable.value:
            config = {
                "home-manager" = {users = Option002 {value = {
                    "${username.value}" = {programs.atuin = {
                        enable = Option003 {value = True}.value
                        enableFishIntegration = Option004 {value = True}.value
                        settings = {
                            auto_sync = Option005 {value = True}.value
                            search_mode = Option006 {value = "fuzzy"}.value
                            sync_frequency = Option007 {value = "5m"}.value
                        }
                    }}
                }}.value}
            }
    }

schema Chrome:
    enable: Option022
    default: Option021
    body: {str:any} = {
        options.myConfig.modules.chrome = {
            enable = _enable("Enable Google Chrome browser")
            default = _option({type = {"$ref" = ["lib", "types", "bool"]}, default = False, description = "Set Chrome as the default browser via MIME types"})
        }
        if enable.value:
            config = {
                environment.systemPackages = Option000 {value = [Package {"$ref" = ["pkgs", "google-chrome"]}]}.value
                xdg.mime = {
                    if default.value:
                        defaultApplications = Option067 {value = {mime: "google-chrome.desktop" for mime in _mime}}.value
                }
            }
    }

schema Godot:
    enable: Option025
    body: {str:any} = {
        options.myConfig.modules.godot.enable = _enable("Godot game engine")
        if enable.value:
            config.environment.systemPackages = Option000 {value = [Package {"$ref" = ["pkgs", "unstable", "godot_4"]}]}.value
    }

schema Bun:
    enable: Option020
    body: {str:any} = {
        options.myConfig.modules.bun.enable = _enable("Bun JavaScript runtime and package manager")
        if enable.value:
            config.environment.systemPackages = Option000 {value = [Package {"$ref" = ["pkgs", "bun"]}]}.value
    }

schema Obsidian:
    enable: Option032
    body: {str:any} = {
        options.myConfig.modules.obsidian.enable = _enable("Obsidian note-taking")
        if enable.value:
            config.environment.systemPackages = Option000 {value = [Package {"$ref" = ["pkgs", "obsidian"]}]}.value
    }
