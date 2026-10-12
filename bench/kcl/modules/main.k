schema Firefox:
    enable: Option024
    default: Option023
    body: {str:any} = {
        options.myConfig.modules.firefox = {
            enable = {
                "$call" = {function = {"$ref" = ["lib", "mkEnableOption"]}, args = ["Enable Firefox browser"]}
            }
            default = {
                "$call" = {function = {"$ref" = ["lib", "mkOption"]}, args = [{type = {"$ref" = ["lib", "types", "bool"]}, default = True, description = "Set Firefox as the default browser via MIME types"}]}
            }
        }
        imports = [{
            if enable.value:
                config = {
                    programs.firefox.enable = Option041 {value = True}.value
                    xdg.mime = {
                        if default.value:
                            defaultApplications = {mime: "firefox.desktop" for mime in ["text/html", "x-scheme-handler/http", "x-scheme-handler/https", "x-scheme-handler/about", "x-scheme-handler/unknown"]}
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
                enable = {
                    "$call" = {function = {"$ref" = ["lib", "mkEnableOption"]}, args = ["Enable Zen Browser"]}
                }
                default = {
                    "$call" = {function = {"$ref" = ["lib", "mkOption"]}, args = [{type = {"$ref" = ["lib", "types", "bool"]}, default = False, description = "Set Zen Browser as the default browser via MIME types"}]}
                }
            }
        }
        imports = [{
            if enable.value:
                config = {
                    environment.systemPackages = [{
                        "$call" = {
                            function = {"$ref" = ["builtins", "getAttr"]}
                            args = ["default", {
                                "$call" = {function = {"$ref" = ["builtins", "getAttr"]}, args = [_system, _packages]}
                            }]
                        }
                    }]
                    xdg.mime = {
                        if default.value:
                            defaultApplications = {mime: "zen.desktop" for mime in ["text/html", "x-scheme-handler/http", "x-scheme-handler/https", "x-scheme-handler/about", "x-scheme-handler/unknown"]}
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
            enable = {
                "$call" = {function = {"$ref" = ["lib", "mkEnableOption"]}, args = ["Kanata keyboard remapping"]}
            }
            devices = {
                "$call" = {function = {"$ref" = ["lib", "mkOption"]}, args = [{type = {
                    "$call" = {function = {"$ref" = ["lib", "types", "listOf"]}, args = [{"$ref" = ["lib", "types", "str"]}]}
                }, default = [], description = "Input device paths for kanata to capture. Find yours with: ls /dev/input/by-id/"}]}
            }
            configFile = {
                "$call" = {function = {"$ref" = ["lib", "mkOption"]}, args = [{type = {"$ref" = ["lib", "types", "path"]}, description = "Path to kanata .kbd config file"}]}
            }
            port = {
                "$call" = {function = {"$ref" = ["lib", "mkOption"]}, args = [{type = {
                    "$call" = {function = {"$ref" = ["lib", "types", "nullOr"]}, args = [{"$ref" = ["lib", "types", "port"]}]}
                }, default = None, description = "TCP port for kanata server (e.g. for glide integration)"}]}
            }
            extraArgs = {
                "$call" = {function = {"$ref" = ["lib", "mkOption"]}, args = [{type = {
                    "$call" = {function = {"$ref" = ["lib", "types", "listOf"]}, args = [{"$ref" = ["lib", "types", "str"]}]}
                }, default = [], description = "Extra command-line args for kanata (e.g. --debug, --log-layer-changes for diagnosing layer/spacebar seizures)"}]}
            }
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
                                    config = {
                                        "$call" = {function = {"$ref" = ["builtins", "readFile"]}, args = [configFile.value]}
                                    }
                                }
                        }
                    }
                    systemd.services = {
                        if len(devices.value) > 0:
                            "kanata-main" = {
                                serviceConfig = {DynamicUser = {
                                    "$call" = {function = {"$ref" = ["lib", "mkForce"]}, args = [False]}
                                }, User = "kanata"}
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
                enable = {
                    "$call" = {function = {"$ref" = ["lib", "mkEnableOption"]}, args = ["Enable user configuration"]}
                }
                username = {
                    "$call" = {function = {"$ref" = ["lib", "mkOption"]}, args = [{type = {"$ref" = ["lib", "types", "str"]}, default = "user", description = "Primary system username (instance binds the real one)"}]}
                }
                email = {
                    "$call" = {function = {"$ref" = ["lib", "mkOption"]}, args = [{type = {"$ref" = ["lib", "types", "str"]}, default = "", description = "Primary git/commit email"}]}
                }
                fullName = {
                    "$call" = {function = {"$ref" = ["lib", "mkOption"]}, args = [{type = {"$ref" = ["lib", "types", "str"]}, default = "", description = "Git author / display name (git user.name)"}]}
                }
                homeDir = {
                    "$call" = {function = {"$ref" = ["lib", "mkOption"]}, args = [{type = {"$ref" = ["lib", "types", "str"]}, default = homeDir.value, description = "User home directory"}]}
                }
                codeDir = {
                    "$call" = {function = {"$ref" = ["lib", "mkOption"]}, args = [{type = {"$ref" = ["lib", "types", "str"]}, default = codeDir.value, description = "Root of source checkouts (the ~/code convention)"}]}
                }
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
                enable = {
                    "$call" = {function = {"$ref" = ["lib", "mkEnableOption"]}, args = [_description]}
                }
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
                                        Environment = [{
                                            "$call" = {function = {"$ref" = ["lib", "concatStrings"]}, args = [["PATH=/run/wrappers/bin:/run/current-system/sw/bin:${homeDir.value}/.local/bin:", {
                                                "$call" = {function = {"$ref" = ["lib", "makeBinPath"]}, args = [_packages.value]}
                                            }]]}
                                        }]
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
