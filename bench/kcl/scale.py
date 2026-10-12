#!/usr/bin/env python3
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULES = {
    "firefox": "Firefox",
    "zen-browser": "ZenBrowser",
    "kanata": "Kanata",
    "users": "Users",
    "lane-gc": "LaneGC",
    "atuin": "Atuin",
    "chrome": "Chrome",
    "godot": "Godot",
    "bun": "Bun",
    "obsidian": "Obsidian",
}


def identities(text, suffix):
    for name in MODULES:
        text = text.replace(f'"{name}"', f'"{name}_{suffix}"')
        text = re.sub(rf"(?<![\w.]){re.escape(name)}(?=\s*=)", f"{name}_{suffix}", text)
    return text


def main(text, suffix):
    text = text.replace("/bench/context.json", "/bench/scaled/context.json")
    text = text.replace("/bench/contract/out/private.json", "/bench/scaled/private.json")
    start = text.index("_sample = {")
    end = text.index("_metadata =", start)
    text = text[:start] + identities(text[start:end], suffix) + text[end:]
    lines = text.splitlines(keepends=True)
    for index, line in enumerate(lines):
        if line.startswith(("_plus =", "_minus =", "_disabled =", "_direct =", "_direct_off =", "_darwin =")):
            lines[index] = identities(line, suffix)
    text = "".join(lines)
    start = text.index("_overrides = {")
    end = text.index("_inputs =", start)
    text = text[:start] + identities(text[start:end], suffix) + text[end:]
    for name in MODULES:
        text = text.replace(f'"{name}-module"', f'"{name}_{suffix}-module"')
    start = text.index("_email =")
    text = text[:start] + identities(text[start:], suffix)
    for name, schema in MODULES.items():
        text = re.sub(rf"\b{schema}(?=\s*\{{)", f"{schema}_{suffix}", text)
        text = text.replace(f"_private[_host].{name}.", f"_private[_host].{name}_{suffix}.")
        text = text.replace(f"_private.whiterabbit.{name}.", f"_private.whiterabbit.{name}_{suffix}.")
        text = text.replace(f"_overrides.{name}.", f"_overrides.{name}_{suffix}.")
        text = re.sub(rf"(?m)^( +){re.escape(name)}(?=\.)", rf"\g<1>{name}_{suffix}", text)
    return text


def modules(text, suffix):
    for name, schema in MODULES.items():
        text = text.replace(f"schema {schema}:", f"schema {schema}_{suffix}:")
        text = text.replace(f"options.myConfig.modules.{name}", f"options.myConfig.modules.{name}_{suffix}")
        text = re.sub(rf'(?m)^            "{re.escape(name)}"(?= = \{{)', f'            "{name}_{suffix}"', text)
    return text


def run():
    source = (ROOT / "bench/kcl/main.k").read_text()
    bodies = (ROOT / "bench/kcl/modules/main.k").read_text()
    types = (ROOT / "bench/schema/kcl/types.k").read_text()
    output = ROOT / "bench/scaled/kcl"
    output.mkdir(parents=True)
    imports = []
    aliases = []
    for clone in range(1, 22):
        suffix = f"{clone:03}"
        package = output / f"clone{suffix}"
        package.mkdir()
        imports_source, host = main(source, suffix).split("\n\n", 1)
        (package / "main.k").write_text(imports_source + "\n\n" + types + "\n" + modules(bodies, suffix) + "\n" + host)
        alias = f"_clone{suffix}"
        imports.append(f"import clone{suffix} as {alias}\n")
        aliases.append(alias)
    combined = "\n" + "enabled = " + " + ".join(f"{alias}.enabled" for alias in aliases) + "\n"
    combined += "modules = " + " | ".join(f"{alias}.modules" for alias in aliases) + "\n"
    combined += "settings = {myConfig.modules = " + " | ".join(f"{alias}.settings.myConfig.modules" for alias in aliases) + "}\n"
    (output / "main.k").write_text("".join(imports) + combined)
    print("clones\tmodules\n21\t210")


if __name__ == "__main__":
    run()
