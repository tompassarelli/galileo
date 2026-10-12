import file
import json

_host = option("host", default="whiterabbit")
assert _host in ["whiterabbit", "ashashi"]
_linux = _host == "whiterabbit"
_context = json.decode(file.read(file.workdir() + "/bench/context.json"))
_private = json.decode(file.read(file.workdir() + "/bench/contract/out/private.json"))
_sample = {
    firefox = {tags = ["browsers"], optins = []}
    "zen-browser" = {tags = [], optins = ["browsers"]}
    kanata = {tags = [], optins = []}
    users = {tags = [], optins = []}
    "lane-gc" = {tags = [], optins = []}
    atuin = {tags = ["terminal"], optins = []}
    chrome = {tags = [], optins = ["browsers"]}
    godot = {tags = ["creative"], optins = []}
    bun = {tags = ["javascript"], optins = []}
    obsidian = {tags = ["notes"], optins = []}
}
_metadata = _context | _sample
_tags = ["terminal", "cli-tools", "desktop", "theming", "auth", "development", "javascript", "python", "database", "rust", "lisp", "racket", "doom-emacs", "browsers", "gaming", "creative", "media", "communication", "office", "notes", "writing", "vpn"] if _linux else ["terminal", "cli-tools", "development"]
_plus = {desktop = ["river"], development = ["claude", "machine-update", "agent-runtime-update", "update-notify", "proton-log-watchdog", "skill-review-queue", "vast-reaper", "socat", "procps", "alsa-lib"], browsers = ["chrome", "qutebrowser", "zen-browser"]} if _linux else {}
_minus = {desktop = ["mako"], development = ["beagle"], database = ["postgresql"], creative = ["godot"], media = ["youtube-music"], office = ["libreoffice"], vpn = ["protonvpn-cli"]} if _linux else {}
_disabled = ["anytype", "bun", "cargo", "clang", "clippy", "clojure", "dbeaver", "drracket", "gcc", "glide", "mail", "nodejs", "obs-studio", "promptfoo", "racket", "rust-analyzer", "rustc", "rustfmt", "slack", "spotify", "wine", "wowup", "zoom"] if _linux else ["bench-shield", "containers", "rebuild-nopasswd"]
_direct = ["users", "nix-settings", "boot", "networking", "remmina", "timezone", "ssh", "swap", "tmp-retention", "auto-upgrade", "framework", "framework13-mic", "fwupd", "thermal-management", "pipewire", "input", "g203-led", "kanata", "neovim", "vscode", "mini-serve", "awscli", "bitwarden-desktop", "cloudflare-auth", "libsecret", "proton-autopurge", "parted", "unixodbc", "nix-ld", "musl", "appimage", "codex", "codex-runtime", "claude", "nh", "agent-slice", "delivery-liveness", "lane-gc", "capacity-watchdog"] if _linux else []
_direct_off = ["bluetooth", "piper", "glide", "guix", "windows-vm"] if _linux else []
_darwin = ["atuin", "bash", "bc", "beagle", "btop", "curl", "delta", "direnv", "dust", "eza", "fastfetch", "fd", "forgejo-cli", "gh", "ghostscript", "ghostty", "git", "gitleaks", "glow", "imagemagick", "jq", "kitty", "procs", "promptfoo", "ripgrep", "starship", "tealdeer", "tree", "unrar", "unzip", "vim", "wget", "yazi", "zoxide"]
_tag_selected = [name for name, meta in _metadata if any tag in _tags {name not in (_minus[tag] if tag in _minus else []) and (tag in meta.tags or (tag in meta.optins and name in (_plus[tag] if tag in _plus else [])))}]
_selected = [name for name in _metadata if (name in _tag_selected or name in _direct) and name not in _disabled + _direct_off and (_linux or name in _darwin)]
_overrides = {
    firefox = {default = True}
} if "firefox" in _selected and "browsers" in _tags and "firefox" not in (_minus.browsers if "browsers" in _minus else []) else {}
_inputs = {nur = ["github:nix-community/NUR"], "zen-browser" = ["github:0xc000022070/zen-browser-flake"]}
assert all name, urls in _inputs {len({url: True for url in urls}) <= 1}

schema Realization:
    enabled: [str]
    required: [str] = []
    software: {str:[str]} = {}

    check:
        all requirement in required {any module in enabled {requirement in (software[module] or [])}}

_responsibilities = [
    ["obsidian", "member_of", "software-units"],
    ["obsidian-module", "member_of", "firn-modules"],
    ["knowledge-authoring-workflow", "requires_software", "obsidian"],
    ["obsidian", "provides_capability", "knowledge-authoring"],
    ["obsidian", "is_realized_by", "obsidian-module"],
] if _linux else []
_realization = Realization {
    enabled = _selected
    required = [triple[2] for triple in _responsibilities if triple[1] == "requires_software"]
    software = {triple[2].replace("-module", ""): [triple[0]] for triple in _responsibilities if triple[1] == "is_realized_by"}
}
_username = Option038 {value = "tom" if _linux else "ashashi"}
_email = Option034 {value = _private[_host].users.email}
_fullName = Option036 {value = "tompassarelli"}
_kanata = {
    configFile = _call(["lib", "concatStrings"], [[{"$ref" = ["flakeRoot"]}, "/dotfiles/kanata/kanata.kbd"]])
    port = 7070
    extraArgs = ["--log-layer-changes"]
    devices = _private.whiterabbit.kanata.devices
}
_firefox = Firefox {enable = Option024 {value = "firefox" in _selected}, default = Option023 {value = _overrides.firefox.default if "firefox" in _overrides else True}}
_zen = ZenBrowser {enable = Option040 {value = "zen-browser" in _selected}, default = Option039 {value = False}}
_keyboard = Kanata {
    enable = Option028 {value = "kanata" in _selected}
    configFile = Option026 {value = _kanata.configFile}
    devices = Option027 {value = _kanata.devices}
    port = Option030 {value = _kanata.port}
    extraArgs = Option029 {value = _kanata.extraArgs}
}
_users = Users {
    enable = Option035 {value = "users" in _selected}
    username = _username
    email = _email
    fullName = _fullName
    homeDir = Option037 {value = "/home/" + _username.value}
    codeDir = Option033 {value = "/home/" + _username.value + "/code"}
}
_gc = LaneGC {enable = Option031 {value = "lane-gc" in _selected}, username = _username, homeDir = _users.homeDir}
_atuin = Atuin {enable = Option019 {value = "atuin" in _selected}, username = _username}
_chrome = Chrome {enable = Option022 {value = "chrome" in _selected}, default = Option021 {value = False}}
_godot = Godot {enable = Option025 {value = "godot" in _selected}}
_bun = Bun {enable = Option020 {value = "bun" in _selected}}
_obsidian = Obsidian {enable = Option032 {value = "obsidian" in _selected}}
_bodies = {firefox = _firefox.body, "zen-browser" = _zen.body, kanata = _keyboard.body, users = _users.body, "lane-gc" = _gc.body, atuin = _atuin.body, chrome = _chrome.body, godot = _godot.body, bun = _bun.body, obsidian = _obsidian.body}
enabled = [name for name in _sample if name in _selected and name in _bodies]
modules = {name: _bodies[name] for name in _sample if _linux or name in _darwin}
settings = {myConfig.modules = {name: {} for name in _sample if name in _selected} | {
    users = {username = _username.value, email = _email.value, fullName = _fullName.value}
    if _linux:
        kanata = _kanata
    if "firefox" in _overrides:
        firefox.default = _call(["lib", "mkDefault"], [_overrides.firefox.default])
}}
