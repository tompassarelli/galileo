package schema

_hosts: {
	whiterabbit: {
		platform: "linux"
		selected: [
			{tag: "terminal"}, {tag: "cli-tools"},
			{tag: "desktop", exclude: ["mako"], optIn: ["river"]},
			{tag: "theming"}, {tag: "auth"},
			{tag: "development", exclude: ["beagle"], optIn: ["claude", "machine-update", "agent-runtime-update", "update-notify", "proton-log-watchdog", "skill-review-queue", "vast-reaper", "socat", "procps", "alsa-lib"]},
			{tag: "javascript"}, {tag: "python"},
			{tag: "database", exclude: ["postgresql"]},
			{tag: "rust"}, {tag: "lisp"}, {tag: "racket"}, {tag: "doom-emacs"},
			{tag: "browsers", optIn: ["chrome", "qutebrowser", "zen-browser"]},
			{tag: "gaming"}, {tag: "creative", exclude: ["godot"]},
			{tag: "media", exclude: ["youtube-music"]},
			{tag: "communication"}, {tag: "office", exclude: ["libreoffice"]},
			{tag: "notes"}, {tag: "writing"}, {tag: "vpn", exclude: ["protonvpn-cli"]},
		]
		disabled: ["anytype", "bun", "cargo", "clang", "clippy", "clojure", "dbeaver", "drracket", "gcc", "glide", "mail", "nodejs", "obs-studio", "promptfoo", "racket", "rust-analyzer", "rustc", "rustfmt", "slack", "spotify", "wine", "wowup", "zoom"]
		direct: ["users", "nix-settings", "boot", "networking", "remmina", "timezone", "ssh", "swap", "tmp-retention", "auto-upgrade", "framework", "framework13-mic", "fwupd", "thermal-management", "pipewire", "input", "g203-led", "kanata", "neovim", "vscode", "mini-serve", "awscli", "bitwarden-desktop", "cloudflare-auth", "libsecret", "proton-autopurge", "parted", "unixodbc", "nix-ld", "musl", "appimage", "codex", "codex-runtime", "claude", "nh", "agent-slice", "delivery-liveness", "lane-gc", "capacity-watchdog"]
		identity: {username: "tom", fullName: "tompassarelli"}
	}
	ashashi: {
		platform: "darwin"
		selected: [{tag: "terminal"}, {tag: "cli-tools"}, {tag: "development"}]
		disabled: ["bench-shield", "containers", "rebuild-nopasswd"]
		direct: []
		identity: {username: "ashashi", fullName: "tompassarelli"}
	}
}

_selection: _hosts[_host] & {
	selected: [...{tag: string, exclude: *[] | [...string], optIn: *[] | [...string]}]
}
_identity: _selection.identity & {email: private[_host].users.email}
_kanataSettings: {
	configFile: #Options["myConfig.modules.kanata.configFile"] & {"$call": {
		function: {"$ref": ["lib", "concatStrings"]}
		args: [[{"$ref": ["flakeRoot"]}, "/dotfiles/kanata/kanata.kbd"]]
	}}
	port: #Options["myConfig.modules.kanata.port"] & 7070
	extraArgs: #Options["myConfig.modules.kanata.extraArgs"] & ["--log-layer-changes"]
	devices: #Options["myConfig.modules.kanata.devices"] & private.whiterabbit.kanata.devices
}
_settings: {
	for m in _sample if _tagActive[m.name] {(m.name): {}}
	users: {
		username: #Options["myConfig.modules.users.username"] & _identity.username
		email:    #Options["myConfig.modules.users.email"] & _identity.email
		fullName: #Options["myConfig.modules.users.fullName"] & _identity.fullName
	}
	if _host == "whiterabbit" {
		kanata: _kanataSettings
		"lane-gc": {}
	}
	for name, values in _overrides {(name): values}
}

_responsibilities: [] | [...[string, string, string]]
if _host == "whiterabbit" {
	_responsibilities: [
		["obsidian", "member_of", "software-units"],
		["obsidian-module", "member_of", "firn-modules"],
		["knowledge-authoring-workflow", "requires_software", "obsidian"],
		["obsidian", "provides_capability", "knowledge-authoring"],
		["obsidian", "is_realized_by", "obsidian-module"],
	]
}
if _host == "ashashi" {_responsibilities: []}
