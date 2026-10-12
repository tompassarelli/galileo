#!/usr/bin/env python3
import json
import re
from pathlib import Path


source = Path(__file__).resolve().parent
output = source.parent / "scaled/nickel"
main = (source / "main.ncl").read_text()
sample = re.search(r"let sample = \[(.*?)\] in", main).group(1)
names = json.loads("[" + sample + "]")
clones = range(1, 22)


def alias(name, clone):
    return f"{name}_{clone:03}"


def identities(text):
    return re.sub(
        r'"([^"\n]*)"',
        lambda match: ", ".join(json.dumps(alias(match[1], clone)) for clone in clones)
        if match[1] in names else match[0],
        text,
    )


def options(text, clone):
    for name in names:
        text = text.replace(f"options.myConfig.modules.{name}", f"options.myConfig.modules.{alias(name, clone)}")
        text = text.replace(f'options.myConfig.modules."{name}"', f'options.myConfig.modules."{alias(name, clone)}"')
    return text


modules = (source / "modules.ncl").read_text()
fields = list(re.finditer(r'^  ("[^"\n]+"|[\w-]+) = fun ', modules, re.M))
blocks = {}
for index, field in enumerate(fields):
    end = fields[index + 1].start() if index + 1 < len(fields) else modules.rfind("}")
    blocks[field[1].strip('"')] = modules[field.start():end]
body = modules[:fields[0].start()].replace("../schema/nickel/types.ncl", "../../schema/nickel/types.ncl")
body += blocks["users_options"]
for clone in clones:
    for name in names:
        block = blocks[name]
        key = json.dumps(name) if block.startswith('  "') else name
        new_key = json.dumps(alias(name, clone)) if block.startswith('  "') else alias(name, clone)
        body += options(block.replace(f"  {key} = fun ", f"  {new_key} = fun ", 1), clone)
body += modules[modules.rfind("}"):]

rules = (source / "rules.ncl").read_text()
start = rules.index("let sample = {") + len("let sample = {\n")
end = rules.index("} in", start)
registry = rules[start:end]
expanded = ""
for clone in clones:
    for line in registry.splitlines(keepends=True):
        match = re.match(r'  ("[^"\n]+"|[\w-]+) = ', line)
        name = match[1].strip('"')
        key = json.dumps(alias(name, clone)) if match[1].startswith('"') else alias(name, clone)
        expanded += line.replace(match[1], key, 1)
rules = rules[:start] + expanded + rules[end:]
start = rules.index("  tag_override = fun host =>\n") + len("  tag_override = fun host =>\n")
end = rules.index("  realization =", start)
override = rules[start:end].removesuffix(",\n")
overrides = []
for clone in clones:
    expression = override.replace('"firefox"', json.dumps(alias("firefox", clone)))
    expression = expression.replace("{firefox.default", "{" + alias("firefox", clone) + ".default")
    overrides.append("(" + expression + ")")
rules = rules[:start] + " & ".join(overrides) + ",\n" + rules[end:]

hosts = (source / "hosts.ncl").read_text()
hosts = hosts.replace("../schema/nickel/types.ncl", "../../schema/nickel/types.ncl")
hosts = hosts.replace("../contract/out/private.json", "../private.json")
hosts = re.sub(r'(let darwin_allowed = \[)(.*?)(\] in)', lambda match: match[1] + identities(match[2]) + match[3], hosts, flags=re.S)
hosts = re.sub(r'\b(plus|minus|disabled|on|off) = \[(.*?)\]', lambda match: match[1] + " = [" + identities(match[2]) + "]", hosts, flags=re.S)
hosts = re.sub(
    r'^      \[.*"(?:' + "|".join(map(re.escape, names)) + r')-module".*\],\n',
    lambda match: "".join(re.sub(r'"([^"\n]+)-module"', lambda fact: json.dumps(alias(fact[1], clone) + "-module") if fact[1] in names else fact[0], match[0]) for clone in clones),
    hosts,
    flags=re.M,
)
start = hosts.index("    settings = {\n") + len("    settings = {\n")
end = re.search(r'^    },\n', hosts[start:], re.M).start() + start
settings = hosts[start:end]
fields = list(re.finditer(r'^      ([\w-]+) = ', settings, re.M))
expanded = ""
for index, field in enumerate(fields):
    block = settings[field.start():fields[index + 1].start() if index + 1 < len(fields) else len(settings)]
    name = field[1]
    if name in names:
        for clone in clones:
            expanded += block.replace(f"      {name} =", f"      {alias(name, clone)} =", 1).replace(f"private.whiterabbit.{name}.", f"private.whiterabbit.{alias(name, clone)}.")
    else:
        expanded += block
hosts = hosts[:start] + expanded + hosts[end:]
hosts = re.sub(
    r'^    settings\.users = .*\n',
    lambda match: "".join(match[0].replace("settings.users =", f"settings.{alias('users', clone)} =", 1).replace("private.ashashi.users.", f"private.ashashi.{alias('users', clone)}.") for clone in clones),
    hosts,
    flags=re.M,
)

main = main.replace("let sample = [" + sample + "] in", "let sample = [" + ", ".join(json.dumps(alias(name, clone)) for clone in clones for name in names) + "] in")
users_for = {alias(name, clone): alias("users", clone) for clone in clones for name in ("lane-gc", "atuin")}
main = main.replace("let export = fun host =>", "let users_for = {" + ", ".join(json.dumps(key) + " = " + json.dumps(value) for key, value in users_for.items()) + "} in\nlet export = fun host =>")
main = main.replace('bodies.users_options (raw "users")', "std.array.map (fun name => bodies.users_options (raw name)) [" + ", ".join(json.dumps(alias("users", clone)) for clone in clones) + "]")
main = main.replace('std.array.elem key ["lane-gc", "atuin"]', "std.record.has_field key users_for")
main = main.replace('(raw "users") else bodies.', '(raw users_for."%{key}") else bodies.')
start = main.index("  let realizers =")
end = main.index("  let realized =", start)
main = main[:start] + '''  let realizing = std.array.filter (fun triple => std.array.at 1 triple == "is_realized_by") facts in
  let realizers = std.record.from_array (std.array.map (fun software => {
    field = software, value = std.array.map (std.array.at 2)
      (std.array.filter (fun triple => std.array.at 0 triple == software) realizing),
  }) (std.array.dedup (std.array.map (std.array.at 0) realizing))) in
''' + main[end:]

output.mkdir(parents=True, exist_ok=True)
for filename, text in (("modules.ncl", body), ("rules.ncl", rules), ("hosts.ncl", hosts), ("main.ncl", main)):
    (output / filename).write_text(text)
print("nickel: generated one program with 210 module definitions")
