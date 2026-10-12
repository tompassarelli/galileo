#!/usr/bin/env python3
import re
from pathlib import Path


SOURCE = Path(__file__).resolve().parent
OUTPUT = SOURCE.parent / "scaled" / "dhall"
CLONES = range(1, 22)
MODULES = {
    "firefox": ("Firefox", "firefox", "ff"),
    "zen-browser": ("Zen", "zen", "zen"),
    "kanata": ("Kanata", "kanata", "kanata"),
    "users": ("Users", "users", "user"),
    "lane-gc": ("Lane", "lane", "lane"),
    "atuin": ("Atuin", "atuin", "atuin"),
    "chrome": ("Chrome", "chrome", "chrome"),
    "godot": ("Godot", "godot", "godot"),
    "bun": ("Bun", "bun", "bun"),
    "obsidian": ("Obsidian", "obsidian", "obsidian"),
}


def alias(name, clone):
    return f"{name}_{clone:03}"


def label(name):
    return f"`{name}`" if "-" in name else name


def names(text, clone):
    for name in MODULES:
        text = text.replace(f"n.Name.{label(name)}", f"n.Name.{label(alias(name, clone))}")
    return text


def locals_(text, clone, bindings):
    pattern = r'"(?:\\.|[^"\\])*"|`[^`]*`|(?<![\w.])\b(' + "|".join(map(re.escape, bindings)) + r")\b"

    def replace(match):
        if not match[1]:
            return match[0]
        if re.match(r"\s*=(?!=)", text[match.end():]) and text[max(0, match.start() - 4):match.start()] != "let ":
            return match[0]
        return alias(match[1], clone)

    return re.sub(pattern, replace, text)


def flags(text):
    for name in MODULES:
        pattern = re.escape(label(name)) + r" = (True|False)"
        text = re.sub(pattern, lambda match: ", ".join(
            f"{label(alias(name, clone))} = {match[1]}" for clone in CLONES
        ), text)
    return text


def registry(text):
    for name in MODULES:
        key = re.escape(f"`{name}`")
        text = re.sub(r"(?<=< )" + key + r"(?= \|)|(?<=\| )" + key + r"(?= \|| >)",
                      " | ".join(f"`{alias(name, clone)}`" for clone in CLONES), text)
        text = re.sub(key + r" : Bool", ", ".join(
            f"`{alias(name, clone)}` : Bool" for clone in CLONES
        ), text)
        text = re.sub(key + r' = "' + re.escape(name) + '"', ", ".join(
            f'`{alias(name, clone)}` = "{alias(name, clone)}"' for clone in CLONES
        ), text)
        text = re.sub(key + r" = False", ", ".join(
            f"`{alias(name, clone)}` = False" for clone in CLONES
        ), text)
    return text


def modules(text):
    header, rest = text.split("let Firefox =", 1)
    blocks, _ = ("let Firefox =" + rest).rsplit("\nin {", 1)
    definitions = []
    fields = []
    for clone in CLONES:
        copy = blocks
        for name, (type_, function, _) in MODULES.items():
            copy = locals_(copy, clone, [type_])
            copy = copy.replace(f"let {function} =", f"let {alias(function, clone)} =")
            copy = copy.replace(f"modules = obj (toMap {{ {label(name)} =",
                                f"modules = obj (toMap {{ {label(alias(name, clone))} =")
            fields.extend(f"{alias(binding, clone)} = {alias(binding, clone)}"
                          for binding in (type_, function))
        definitions.append(copy)
    return header + "\n".join(definitions) + "\nin { " + ", ".join(fields) + " }\n"


def main(text):
    header, rest = text.split("let firefoxModule =", 1)
    declarations, rest = ("let firefoxModule =" + rest).split("let darwin =", 1)
    metadata = [function + "Module" for _, function, _ in MODULES.values()
                if function not in ("kanata", "users", "lane")]
    prefix = header
    for clone in CLONES:
        prefix += locals_(names(declarations, clone), clone, metadata + ["sample"])
    prefix += "let sample = " + " # ".join(alias("sample", clone) for clone in CLONES) + "\n"
    hosts, rest = ("let darwin =" + rest).split("let export =", 1)
    responsibility, hosts_end = hosts.split("let responsibilities =", 1)
    responsibility_body, hosts_end = ("let responsibilities =" + hosts_end).split("let whiterabbit =", 1)
    prefix += responsibility
    for clone in CLONES:
        prefix += locals_(names(responsibility_body, clone), clone, ["responsibilities"])
    hosts = "let whiterabbit =" + hosts_end
    hosts = hosts.replace("responsibilities.requiredSoftware", "responsibilities_001.requiredSoftware")
    hosts = hosts.replace("private.whiterabbit.users.email", "private.whiterabbit.users_001.email")
    hosts = hosts.replace("private.ashashi.users.email", "private.ashashi.users_001.email")
    prefix = flags(prefix) + flags(hosts)
    export, ending = ("let export =" + rest).split("let whiterabbitExport =", 1)
    export_header, realized = export.split("  let realized =", 1)
    realized, export_rest = ("  let realized =" + realized).split("  let urls =", 1)
    urls, export_rest = ("  let urls =" + export_rest).split("  let tagOverride :", 1)
    configs, export_rest = ("  let tagOverride :" + export_rest).split("  let bodies =", 1)
    bodies, export_rest = ("  let bodies =" + export_rest).split("  let enabled =", 1)
    enabled, settings = ("  let enabled =" + export_rest).split("  let settings =", 1)
    body_items = bodies.split("    [", 1)[1].split(" ]\n", 1)[0]
    body_fold = bodies.split(" ]\n", 1)[1]
    settings_fields = settings.split("    ( ", 1)[1].split("    ) }) })", 1)[0]
    bindings = [value[2] for value in MODULES.values()] + ["tagOverride", "configFile", "port", "portBound"]
    output = prefix + export_header + urls
    body_copies = []
    for clone in CLONES:
        realization = names(realized, clone).replace("n.none // { obsidian = True }", f"n.none // {{ obsidian_{clone:03} = True }}")
        output += locals_(realization, clone, metadata + ["realized", "responsibilities"])
        copy = names(configs, clone)
        for _, (type_, function, _) in MODULES.items():
            copy = copy.replace(f"m.{type_}", f"m.{alias(type_, clone)}")
        copy = copy.replace("private.whiterabbit.kanata.devices", f"private.whiterabbit.kanata_{clone:03}.devices")
        output += locals_(copy, clone, bindings + metadata)
        items = names(body_items, clone)
        for _, (_, function, _) in MODULES.items():
            items = items.replace(f"m.{function} ", f"m.{alias(function, clone)} ")
        body_copies.append(locals_(items, clone, bindings))
        fields = locals_(settings_fields, clone, bindings + ["sample"])
        fields = names(fields, clone)
        for name in MODULES:
            fields = fields.replace(f'mapKey = "{name}"', f'mapKey = "{alias(name, clone)}"')
            fields = fields.replace(f"{label(name)} = True", f"{label(alias(name, clone))} = True")
        output += f"  let settingsFields_{clone:03} =\n    ( {fields}    )\n"
    output += "  let realized = " + " && ".join(alias("realized", clone) for clone in CLONES) + "\n"
    output += "  let bodies = List/fold { name : n.Name, body : j.JSON }\n    [" + ",".join(body_copies) + " ]\n" + body_fold
    output += enabled
    output += "  let settings = obj (toMap { myConfig = obj (toMap { modules = obj ( "
    output += " # ".join(f"settingsFields_{clone:03}" for clone in CLONES) + " ) }) })\n"
    output += settings.split("    ) }) })", 1)[1]
    return output + "let whiterabbitExport =" + ending


def run():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    for filename in ("json.dhall", "context-type.dhall"):
        text = (SOURCE / filename).read_text()
        (OUTPUT / filename).write_text(text.replace("../schema/dhall/types.dhall", "../../schema/dhall/types.dhall"))
    text = (SOURCE / "private-type.dhall").read_text()
    for name in ("users", "kanata"):
        pattern = r"\b" + name + r" : (\{[^}]+\})"
        text = re.sub(pattern, lambda match: ", ".join(
            f"{alias(name, clone)} : {match[1]}" for clone in CLONES
        ), text)
    (OUTPUT / "private-type.dhall").write_text(text.replace("../schema/dhall/types.dhall", "../../schema/dhall/types.dhall"))
    (OUTPUT / "names.dhall").write_text(registry((SOURCE / "names.dhall").read_text()))
    for filename, generate in (("modules.dhall", modules), ("main.dhall", main)):
        text = generate((SOURCE / filename).read_text())
        (OUTPUT / filename).write_text(text.replace("../schema/dhall/types.dhall", "../../schema/dhall/types.dhall"))


if __name__ == "__main__":
    run()
