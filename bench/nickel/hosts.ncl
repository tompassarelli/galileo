let types = import "../schema/nickel/types.ncl" in
let private = import "../contract/out/private.json" in
let tag = fun tag_name => {name = tag_name, plus = [], minus = []} in
let darwin_allowed = [
  "atuin", "bash", "bc", "beagle", "btop", "curl", "delta", "direnv", "dust", "eza",
  "fastfetch", "fd", "forgejo-cli", "gh", "ghostscript", "ghostty", "git", "gitleaks",
  "glow", "imagemagick", "jq", "kitty", "procs", "promptfoo", "ripgrep", "starship",
  "tealdeer", "tree", "unrar", "unzip", "vim", "wget", "yazi", "zoxide",
] in
{
  whiterabbit = {
    platform = "linux", allowed = darwin_allowed,
    responsibilities = [
      ["obsidian", "member_of", "software-units"],
      ["obsidian-module", "member_of", "firn-modules"],
      ["knowledge-authoring-workflow", "requires_software", "obsidian"],
      ["obsidian", "provides_capability", "knowledge-authoring"],
      ["obsidian", "is_realized_by", "obsidian-module"],
    ],
    tags = [
      tag "terminal", tag "cli-tools",
      {name = "desktop", minus = ["mako"], plus = ["river"]},
      tag "theming", tag "auth",
      {name = "development", minus = ["beagle"], plus = [
        "claude", "machine-update", "agent-runtime-update", "update-notify", "proton-log-watchdog",
        "skill-review-queue", "vast-reaper", "socat", "procps", "alsa-lib",
      ]},
      tag "javascript", tag "python",
      {name = "database", minus = ["postgresql"], plus = []},
      tag "rust", tag "lisp", tag "racket", tag "doom-emacs",
      {name = "browsers", minus = [], plus = ["chrome", "qutebrowser", "zen-browser"]},
      tag "gaming", {name = "creative", minus = ["godot"], plus = []},
      {name = "media", minus = ["youtube-music"], plus = []},
      tag "communication", {name = "office", minus = ["libreoffice"], plus = []},
      tag "notes", tag "writing", {name = "vpn", minus = ["protonvpn-cli"], plus = []},
    ],
    disabled = [
      "anytype", "bun", "cargo", "clang", "clippy", "clojure", "dbeaver", "drracket", "gcc",
      "glide", "mail", "nodejs", "obs-studio", "promptfoo", "racket", "rust-analyzer", "rustc",
      "rustfmt", "slack", "spotify", "wine", "wowup", "zoom",
    ],
    on = [
      "users", "nix-settings", "boot", "networking", "remmina", "timezone", "ssh", "swap",
      "tmp-retention", "auto-upgrade", "framework", "framework13-mic", "fwupd",
      "thermal-management", "pipewire", "input", "g203-led", "kanata", "neovim", "vscode",
      "mini-serve", "awscli", "bitwarden-desktop", "cloudflare-auth", "libsecret",
      "proton-autopurge", "parted", "unixodbc", "nix-ld", "musl", "appimage", "codex",
      "codex-runtime", "claude", "nh", "agent-slice", "delivery-liveness", "lane-gc", "capacity-watchdog",
    ],
    off = ["bluetooth", "piper", "glide", "guix", "windows-vm"],
    settings = {
      users = {username = "tom", email | types."myConfig.modules.users.email" = private.whiterabbit.users.email, fullName = "tompassarelli"},
      kanata = {
        configFile = {"$call" = {function = {"$ref" = ["lib", "concatStrings"]}, args = [[
          {"$ref" = ["flakeRoot"]}, "/dotfiles/kanata/kanata.kbd",
        ]]}},
        port = 7070,
        extraArgs = ["--log-layer-changes"],
        devices | types."myConfig.modules.kanata.devices" = private.whiterabbit.kanata.devices,
      },
    },
  },
  ashashi = {
    platform = "darwin", allowed = darwin_allowed,
    tags = [tag "terminal", tag "cli-tools", tag "development"],
    disabled = ["bench-shield", "containers", "rebuild-nopasswd"],
    on = [], off = [],
    settings.users = {username = "ashashi", email | types."myConfig.modules.users.email" = private.ashashi.users.email, fullName = "tompassarelli"},
  },
}
