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
_optins = ["river", "claude", "machine-update", "agent-runtime-update", "update-notify", "proton-log-watchdog", "skill-review-queue", "vast-reaper", "socat", "procps", "alsa-lib", "chrome", "qutebrowser", "zen-browser"] if _linux else []
_excluded = ["mako", "beagle", "postgresql", "godot", "youtube-music", "libreoffice", "protonvpn-cli"] if _linux else []
_disabled = ["anytype", "bun", "cargo", "clang", "clippy", "clojure", "dbeaver", "drracket", "gcc", "glide", "mail", "nodejs", "obs-studio", "promptfoo", "racket", "rust-analyzer", "rustc", "rustfmt", "slack", "spotify", "wine", "wowup", "zoom"] if _linux else ["bench-shield", "containers", "rebuild-nopasswd"]
_direct = ["users", "nix-settings", "boot", "networking", "remmina", "timezone", "ssh", "swap", "tmp-retention", "auto-upgrade", "framework", "framework13-mic", "fwupd", "thermal-management", "pipewire", "input", "g203-led", "kanata", "neovim", "vscode", "mini-serve", "awscli", "bitwarden-desktop", "cloudflare-auth", "libsecret", "proton-autopurge", "parted", "unixodbc", "nix-ld", "musl", "appimage", "codex", "codex-runtime", "claude", "nh", "agent-slice", "delivery-liveness", "lane-gc", "capacity-watchdog"] if _linux else []
_direct_off = ["bluetooth", "piper", "glide", "guix", "windows-vm"] if _linux else []
_darwin = ["atuin", "bash", "bc", "beagle", "btop", "curl", "delta", "direnv", "dust", "eza", "fastfetch", "fd", "forgejo-cli", "gh", "ghostscript", "ghostty", "git", "gitleaks", "glow", "imagemagick", "jq", "kitty", "procs", "promptfoo", "ripgrep", "starship", "tealdeer", "tree", "unrar", "unzip", "vim", "wget", "yazi", "zoxide"]
_selected = [name for name, meta in _metadata if (any tag in meta.tags {tag in _tags} or (name in _optins and any tag in meta.optins {tag in _tags}) or name in _direct) and name not in _excluded + _disabled + _direct_off and (_linux or name in _darwin)]
_overrides = {
    firefox = {default = True}
} if "browsers" in _tags else {}
_inputs = {nur = ["github:nix-community/NUR"], "zen-browser" = ["github:0xc000022070/zen-browser-flake"]}
assert all name, urls in _inputs {len({url: True for url in urls}) <= 1}

schema Realization:
    enabled: [str]
    required: [str] = []
    software: {str:[str]} = {}

    check:
        all requirement in required {any module in enabled {requirement in (software[module] or [])}}

_realization = Realization {enabled = _selected}
_names = ["firefox", "zen-browser", "kanata", "users", "lane-gc", "atuin", "chrome", "godot", "bun", "obsidian"]
_username = Option038 {value = "tom" if _linux else "ashashi"}
_email = Option034 {value = _private[_host].users.email}
_fullName = Option036 {value = "tompassarelli"}
_kanata = {
    configFile = {
        "$call" = {function = {"$ref" = ["lib", "concatStrings"]}, args = [[{"$ref" = ["flakeRoot"]}, "/dotfiles/kanata/kanata.kbd"]]}
    }
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
_bodies = {firefox = _firefox.body, "zen-browser" = _zen.body, kanata = _keyboard.body, users = _users.body, "lane-gc" = _gc.body}
enabled = [name for name in _names if name in _selected and name in _bodies]
modules = {name: _bodies[name] for name in enabled}
settings = {myConfig.modules = {name: {} for name in _names if name in _selected} | {
    users = {username = _username.value, email = _email.value, fullName = _fullName.value}
    if _linux:
        kanata = _kanata
    if "firefox" in _overrides:
        firefox.default = {
            "$call" = {function = {"$ref" = ["lib", "mkDefault"]}, args = [_overrides.firefox.default]}
        }
}}
