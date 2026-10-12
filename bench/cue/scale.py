#!/usr/bin/env python3
import re
from pathlib import Path


root = Path(__file__).resolve().parents[2]
source = root / "bench/cue"
out = root / "bench/scaled/cue"
rules = (source / "rules.cue").read_text()
sample = rules.split("_sample: [\n", 1)[1].split("\n]", 1)[0]
names = re.findall(r'\{name: "([^"]+)"', sample)


def renamed(text, clone, filename):
    aliases = {name: f"{name}_{clone:03}" for name in names}

    def name(value):
        return aliases.get(value, value)

    def label(value):
        return f'"{name(value[1:-1])}"' if value.startswith('"') else name(value)

    def selections(match):
        return re.sub(r'"([^"]+)"', lambda item: f'"{name(item[1])}"', match[0])

    text = re.sub(
        r'(_active|_settings)\.([A-Za-z][\w-]*)',
        lambda match: f"{match[1]}.{name(match[2])}",
        text,
    )
    text = re.sub(
        r'(_active|_settings)\["([^"]+)"\]',
        lambda match: f'{match[1]}["{name(match[2])}"]',
        text,
    )
    text = re.sub(
        r'(options: myConfig: modules: )("[^"\n]+"|[\w-]+)(:)',
        lambda match: match[1] + label(match[2]) + match[3],
        text,
    )
    if filename == "main.cue":
        text = re.sub(
            r'(\b)(users|kanata)(:)',
            lambda match: match[1] + name(match[2]) + match[3],
            text,
        )
    elif filename == "hosts.cue":
        text = re.sub(r'\b(exclude|optIn|disabled|direct): \[[^\]]*\]', selections, text)
        text = re.sub(
            r'(private(?:\[_host\]|\.whiterabbit)\.)(users|kanata)\b',
            lambda match: match[1] + name(match[2]),
            text,
        )
        text = re.sub(
            r'(^\t+)(users|kanata|"lane-gc")(:)',
            lambda match: match[1] + label(match[2]) + match[3],
            text,
            flags=re.MULTILINE,
        )
        text = text.replace('"obsidian-module"', f'"{name("obsidian")}-module"')
    elif filename == "modules.cue":
        text = re.sub(
            r'(^\t)("[^"\n]+"|[\w-]+)(:)',
            lambda match: match[1] + label(match[2]) + match[3],
            text,
            flags=re.MULTILINE,
        )
    elif filename == "rules.cue":
        text = re.sub(
            r'(\{(?:name|module): )"([^"]+)"',
            lambda match: match[1] + f'"{name(match[2])}"',
            text,
        )
        text = re.sub(r'_allowed: \[[^\]]*\]', selections, text)
        text = text.replace("_tagOverrides: firefox:", f'_tagOverrides: {name("firefox")}:')
        text = text.replace("_tagOverrides.firefox.", f'_tagOverrides.{name("firefox")}.')
    return text


out.mkdir(parents=True, exist_ok=True)
parts = ['package schema\n\nimport (\n"list"\n"strings"\n)\n',
         'context: _\nprivate: _\n_sharedContext: context\n_sharedPrivate: private\n']
for clone in range(1, 22):
    parts.append(f'_clones: "{clone:03}": {{\ncontext: _sharedContext\nprivate: _sharedPrivate\n')
    for filename in ("main.cue", "hosts.cue", "modules.cue", "rules.cue"):
        text = (source / filename).read_text().removeprefix("package schema\n")
        text = re.sub(r'\nimport \(.*?\n\)\n', "\n", text, count=1, flags=re.DOTALL)
        parts.append(renamed(text, clone, filename))
    parts.append("}\n")
parts.append('''
export: {
    enabled: list.Concat([for clone in _clones {clone.export.enabled}])
    settings: myConfig: modules: {
        for clone in _clones
        for name, values in clone.export.settings.myConfig.modules {(name): values}
    }
    modules: {
        for clone in _clones
        for name, body in clone.export.modules {(name): body}
    }
}
''')
for field in ("_realization", "_inputs", "_contributions", "_active", "_overrides"):
    parts.append(field + ": {for name, clone in _clones {(name): clone." + field + "}}\n")
(out / "main.cue").write_text("\n".join(parts))
print(f"clones\t21\nmodules\t{21 * len(names)}")
