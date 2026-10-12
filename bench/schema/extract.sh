#!/usr/bin/env bash
set -euo pipefail

if (( $# != 0 )); then
  printf 'usage: FIRN_ROOT=<pin> GALILEO=<binary> %s\n' "$0" >&2
  exit 2
fi
: "${FIRN_ROOT:?Set FIRN_ROOT to the firn pin 10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9}"
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
"$repo/bench/sample-check.sh" >&2
mkdir -p "$repo/target"
work=$(mktemp -d "$repo/target/schema.XXXXXX")
trap 'rm -f -- "$work/full.json"; rmdir -- "$work"' EXIT
TMPDIR="$work" "${GALILEO:-galileo}" schema extract --host whiterabbit --flake "$FIRN_ROOT" --out "$work/full.json"
PYTHONDONTWRITEBYTECODE=1 python3 - "$repo/bench/sample.tsv" "$work/full.json" "$repo/bench/schema/sample.json" <<'PY'
import csv
import json
import sys
from pathlib import Path

manifest, extracted, output = map(Path, sys.argv[1:])
with manifest.open(newline="") as stream:
    units = {row["id"]: row for row in csv.DictReader(stream, delimiter="\t")}
modules = set()
for row in units.values():
    parts = Path(row["pin_file"]).parts
    if parts[0] == "modules":
        modules.add(parts[1])
    elif parts[:2] == ("native", "nix"):
        modules.add(Path(parts[2]).stem)
prefixes = {f"myConfig.modules.{module}" for module in modules}
settings = {
    "firefox.implementation": ["programs.firefox.enable", "xdg.mime.defaultApplications"],
    "zen-browser.implementation": ["environment.systemPackages", "xdg.mime.defaultApplications"],
    "chrome.implementation": ["environment.systemPackages", "xdg.mime.defaultApplications"],
    "atuin.implementation": ["home-manager.users"],
    "bun.implementation": ["environment.systemPackages"],
    "godot.implementation": ["environment.systemPackages"],
    "obsidian.implementation": ["environment.systemPackages"],
    "kanata.implementation": [
        "hardware.uinput.enable", "services.udev.extraRules", "users.groups", "users.users",
        "services.kanata.enable", "services.kanata.package", "services.kanata.keyboards", "systemd.services",
    ],
    "users.implementation": ["users.users", "security.sudo.extraConfig", "systemd.tmpfiles.rules"],
    "lane-gc.implementation": ["home-manager.users"],
}
nested = {
    "kanata.implementation": {
        "users.groups": ["uinput", "kanata"],
        "users.users": ["kanata.isSystemUser", "kanata.group", "kanata.extraGroups"],
        "services.kanata.keyboards": ["main.devices", "main.port", "main.extraDefCfg", "main.extraArgs", "main.config"],
        "systemd.services": ["kanata-main.serviceConfig.DynamicUser", "kanata-main.serviceConfig.User"],
    },
    "users.implementation": {
        "users.users": ["<name>.shell", "<name>.isNormalUser", "<name>.home", "<name>.extraGroups"],
    },
    "atuin.implementation": {
        "home-manager.users": [
            "<name>.programs.atuin.enable", "<name>.programs.atuin.enableFishIntegration",
            "<name>.programs.atuin.settings.auto_sync", "<name>.programs.atuin.settings.sync_frequency",
            "<name>.programs.atuin.settings.search_mode",
        ],
    },
    "lane-gc.implementation": {
        "home-manager.users": [
            "<name>.systemd.user.services.lane-gc.Unit.Description",
            "<name>.systemd.user.services.lane-gc.Unit.OnFailure",
            "<name>.systemd.user.services.lane-gc.Service.Type",
            "<name>.systemd.user.services.lane-gc.Service.Nice",
            "<name>.systemd.user.services.lane-gc.Service.TimeoutStartSec",
            "<name>.systemd.user.services.lane-gc.Service.Environment",
            "<name>.systemd.user.services.lane-gc.Service.ExecStart",
            "<name>.systemd.user.timers.lane-gc.Unit.Description",
            "<name>.systemd.user.timers.lane-gc.Timer.OnBootSec",
            "<name>.systemd.user.timers.lane-gc.Timer.OnUnitActiveSec",
            "<name>.systemd.user.timers.lane-gc.Install.WantedBy",
        ],
    },
}
mime = {"xdg.mime.defaultApplications": [
    '"text/html"', '"x-scheme-handler/http"', '"x-scheme-handler/https"',
    '"x-scheme-handler/about"', '"x-scheme-handler/unknown"',
]}
for unit in ("firefox.implementation", "zen-browser.implementation", "chrome.implementation"):
    nested[unit] = mime
schema = json.loads(extracted.read_text())
by_name = {option["name"]: option for option in schema}
required = {path for unit, paths in settings.items() if unit in units for path in paths}
missing = required - by_name.keys()
if missing:
    raise SystemExit(f"schema extract: missing options: {', '.join(sorted(missing))}")
selected = {
    name: option for name, option in by_name.items()
    if name in required or any(name.startswith(prefix + ".") for prefix in prefixes)
}
for prefix in prefixes:
    if not any(name.startswith(prefix + ".") for name in selected):
        raise SystemExit(f"schema extract: no options for {prefix}")
native_count = len(selected)
for unit, containers in nested.items():
    if unit not in units:
        continue
    for container, paths in containers.items():
        element = by_name[container].get("inner", {"t": "unknown"})
        for path in paths:
            name = f"{container}.{path}"
            if name not in by_name:
                selected[name] = {"name": name, "container": container, **element}
unknown = sorted(name for name, option in selected.items() if option["t"] == "unknown")
output.write_text(json.dumps([selected[name] for name in sorted(selected)], indent=2) + "\n")
print(f"full records: {len(schema)}; native sample records: {native_count}; nested sample records: {len(selected) - native_count}; untyped nested paths: {len(unknown)}")
print("DECISION AF.3: nested paths left untyped: " + ", ".join(unknown))
PY
