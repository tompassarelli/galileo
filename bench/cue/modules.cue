package schema

#Enable: {
	_text: string
	"$call": {function: {"$ref": ["lib", "mkEnableOption"]}, args: [_text]}
}
#Option: {
	_arg: {...}
	"$call": {function: {"$ref": ["lib", "mkOption"]}, args: [_arg]}
}

#Firefox: {
	enable:  #Options["myConfig.modules.firefox.enable"]
	default: #Options["myConfig.modules.firefox.default"]
}
#Zen: {
	enable:  #Options["myConfig.modules.zen-browser.enable"]
	default: #Options["myConfig.modules.zen-browser.default"]
}
#Kanata: {
	enable:     #Options["myConfig.modules.kanata.enable"]
	configFile: #Options["myConfig.modules.kanata.configFile"]
	devices:    #Options["myConfig.modules.kanata.devices"]
	port:       #Options["myConfig.modules.kanata.port"]
	extraArgs:  #Options["myConfig.modules.kanata.extraArgs"]
}
#Users: {
	enable:   #Options["myConfig.modules.users.enable"]
	username: #Options["myConfig.modules.users.username"]
	email:    #Options["myConfig.modules.users.email"]
	fullName: #Options["myConfig.modules.users.fullName"]
	homeDir:  #Options["myConfig.modules.users.homeDir"]
	codeDir:  #Options["myConfig.modules.users.codeDir"]
}
#LaneGC: {enable: #Options["myConfig.modules.lane-gc.enable"]}

_firefox: #Firefox & {enable: _active.firefox, default: _firefoxDefault}
_zen: #Zen & {enable: _active["zen-browser"], default: false}
_kanata: #Kanata & {
	enable:     _active.kanata
	configFile: _kanataSettings.configFile
	devices:    _kanataSettings.devices
	port:       _kanataSettings.port
	extraArgs:  _kanataSettings.extraArgs
}
_users: #Users & {
	enable:   _active.users
	username: _identity.username
	email:    _identity.email
	fullName: _identity.fullName
	homeDir:  "/home/\(_identity.username)"
	codeDir:  "\(homeDir)/code"
}
_laneGC: #LaneGC & {enable: _active["lane-gc"]}

_mime: ["text/html", "x-scheme-handler/http", "x-scheme-handler/https", "x-scheme-handler/about", "x-scheme-handler/unknown"]
_zenPackages: #Package & {"$ref": ["inputs", "zen-browser", "packages"]}
_system: #Package & {"$ref": ["pkgs", "stdenv", "hostPlatform", "system"]}
_zenPackage: {"$call": {
	function: {"$ref": ["builtins", "getAttr"]}
	args: ["default", {"$call": {
		function: {"$ref": ["builtins", "getAttr"]}
		args: [_system, _zenPackages]
	}}]
}}

_bodies: {
	firefox: {
		options: myConfig: modules: firefox: {
			enable: #Enable & {_text: "Enable Firefox browser"}
			default: #Option & {_arg: {
				type: {"$ref": ["lib", "types", "bool"]}
				default:     true
				description: "Set Firefox as the default browser via MIME types"
			}}
		}
		imports: [{
			if _firefox.enable {
				config: {
					programs: firefox: enable: #Options["programs.firefox.enable"] & true
					xdg: mime: {
						if _firefox.default {
							defaultApplications: #Options["xdg.mime.defaultApplications"] & {
								for key in _mime {(key): "firefox.desktop"}
							}
						}
					}
				}
			}
		}]
	}
	"zen-browser": {
		options: myConfig: modules: "zen-browser": {
			enable: #Enable & {_text: "Enable Zen Browser"}
			default: #Option & {_arg: {
				type: {"$ref": ["lib", "types", "bool"]}
				default:     false
				description: "Set Zen Browser as the default browser via MIME types"
			}}
		}
		imports: [{
			if _zen.enable {
				config: {
					environment: systemPackages: [_zenPackage]
					xdg: mime: {
						if _zen.default {
							defaultApplications: #Options["xdg.mime.defaultApplications"] & {
								for key in _mime {(key): "zen.desktop"}
							}
						}
					}
				}
			}
		}]
	}
	kanata: {
		options: myConfig: modules: kanata: {
			enable: #Enable & {_text: "Kanata keyboard remapping"}
			devices: #Option & {_arg: {
				type: {"$call": {function: {"$ref": ["lib", "types", "listOf"]}, args: [{"$ref": ["lib", "types", "str"]}]}}
				default: []
				description: "Input device paths for kanata to capture. Find yours with: ls /dev/input/by-id/"
			}}
			configFile: #Option & {_arg: {type: {"$ref": ["lib", "types", "path"]}, description: "Path to kanata .kbd config file"}}
			port: #Option & {_arg: {
				type: {"$call": {function: {"$ref": ["lib", "types", "nullOr"]}, args: [{"$ref": ["lib", "types", "port"]}]}}
				default:     null
				description: "TCP port for kanata server (e.g. for glide integration)"
			}}
			extraArgs: #Option & {_arg: {
				type: {"$call": {function: {"$ref": ["lib", "types", "listOf"]}, args: [{"$ref": ["lib", "types", "str"]}]}}
				default: []
				description: "Extra command-line args for kanata (e.g. --debug, --log-layer-changes for diagnosing layer/spacebar seizures)"
			}}
		}
		imports: [{
			if _kanata.enable {
				config: {
					hardware: uinput: enable: #Options["hardware.uinput.enable"] & true
					services: {
						udev: extraRules: #Options["services.udev.extraRules"] & "KERNEL==\"uinput\", MODE=\"0660\", GROUP=\"uinput\", OPTIONS+=\"static_node=uinput\"\n\n"
						kanata: {
							enable: #Options["services.kanata.enable"] & true
							package: #Options["services.kanata.package"] & {"$ref": ["pkgs", "unstable", "kanata"]}
							if len(_kanata.devices) > 0 {
								keyboards: #Options["services.kanata.keyboards"] & {main: {
									devices: _kanata.devices
									if _kanata.port != null {port: _kanata.port}
									extraDefCfg: "process-unmapped-keys yes"
									extraArgs:   _kanata.extraArgs
									config: {"$call": {function: {"$ref": ["builtins", "readFile"]}, args: [_kanata.configFile]}}
								}}
							}
						}
					}
					users: {
						groups: #Options["users.groups"] & {uinput: {}, kanata: {}}
						users: #Options["users.users"] & {kanata: {isSystemUser: true, group: "kanata", extraGroups: ["input", "uinput"]}}
					}
					if len(_kanata.devices) > 0 {
						systemd: services: #Options["systemd.services"] & {"kanata-main": serviceConfig: {
							DynamicUser: {"$call": {function: {"$ref": ["lib", "mkForce"]}, args: [false]}}
							User: "kanata"
						}}
					}
				}
			}
		}]
	}
	users: imports: [{
		options: myConfig: modules: users: {
			enable: #Enable & {_text: "Enable user configuration"}
			username: #Option & {_arg: {type: {"$ref": ["lib", "types", "str"]}, default: "user", description: "Primary system username (instance binds the real one)"}}
			email: #Option & {_arg: {type: {"$ref": ["lib", "types", "str"]}, default: "", description: "Primary git/commit email"}}
			fullName: #Option & {_arg: {type: {"$ref": ["lib", "types", "str"]}, default: "", description: "Git author / display name (git user.name)"}}
			homeDir: #Option & {_arg: {type: {"$ref": ["lib", "types", "str"]}, default: _users.homeDir, description: "User home directory"}}
			codeDir: #Option & {_arg: {type: {"$ref": ["lib", "types", "str"]}, default: _users.codeDir, description: "Root of source checkouts (the ~/code convention)"}}
		}
		if _users.enable {
			config: {
				users: users: #Options["users.users"] & {(_users.username): {
					shell: {"$ref": ["pkgs", "bashInteractive"]}
					isNormalUser: true
					home:         _users.homeDir
					extraGroups: ["wheel", "networkmanager", "plugdev"]
				}}
				security: sudo: extraConfig: #Options["security.sudo.extraConfig"] & "Defaults timestamp_timeout=30\nDefaults timestamp_type=global\n\n"
				systemd: tmpfiles: rules: #Options["systemd.tmpfiles.rules"] & [
					for path in ["Documents", "Pictures/Screenshots", "code", "src"] {"d \(_users.homeDir)/\(path) 0755 \(_users.username) users -"},
				]
			}
		}
	}]
	"lane-gc": {
		options: myConfig: modules: "lane-gc": enable: #Enable & {_text: _laneDescription}
		if _laneGC.enable {
			config: "home-manager": users: #Options["home-manager.users"] & {(_users.username): systemd: user: {
				services: "lane-gc": {
					Unit: {Description: _laneDescription, OnFailure: ["update-notify@%n.service"]}
					Service: {
						Type:            "oneshot"
						Nice:            10
						TimeoutStartSec: "20min"
						Environment: [{"$call": {
							function: {"$ref": ["lib", "concatStrings"]}
							args: [["PATH=/run/wrappers/bin:/run/current-system/sw/bin:\(_users.homeDir)/.local/bin:", {"$call": {
								function: {"$ref": ["lib", "makeBinPath"]}
								args: [[for name in ["bun", "git", "openssh", "gh", "coreutils"] {#Package & {"$ref": ["pkgs", name]}}]]
							}}]]
						}}]
						ExecStart: "\(_users.homeDir)/.local/bin/lane-gc"
					}
				}
				timers: "lane-gc": {
					Unit: Description: _laneDescription
					Timer: {OnBootSec: "10min", OnUnitActiveSec: "1h"}
					Install: WantedBy: ["timers.target"]
				}
			}}
		}
	}
}
_laneDescription: "lane-gc: hourly retire landed, clean, idle worktrees and branches in ~/code containers; report unlanded work"
