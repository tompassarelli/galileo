#!/usr/bin/env python3
import argparse
import re
from pathlib import Path


parser = argparse.ArgumentParser(description='Generate one 210-module Galileo program.')
parser.parse_args()
root = Path(__file__).resolve().parents[2]
source = root / 'bench/galileo/out'
out = root / 'bench/scaled/galileo'
names = sorted(path.name for path in (source / 'modules').iterdir() if path.is_dir())
if len(names) != 10:
    raise SystemExit('galileo scale: expected exactly ten sample modules')
if out.exists():
    raise SystemExit('galileo scale: output directory already exists')
module = '|'.join(re.escape(name) for name in names)
identity = rf'(?:{module})'
tokens = re.compile(r'((?:myConfig|"myConfig")\.(?:modules|"modules")\.)("' + identity
                    + r'"|' + identity + r'\b)|"(?:\\.|[^"\\])*"|#[^\n]*|\./('
                    + identity + r')\.nix|\b(' + identity + r')-module\b')
facts = re.compile(r'^(\s*)(' + identity + r')(?= (?:is a Module|is tagged |can opt into |needs flake input |supports )|\s*$)'
                   + r'|\b((?:adds|drops|disables|turns on|turns off|is realized by) )(' + identity + r')\b'
                   + r'|\b(for )(' + identity + r')(?=\s*$)')


def alias(name, clone):
    return f'{name}_{clone:03}'


def rename(text, clone):
    def token(match):
        if match[1]:
            name = alias(match[2].strip('"'), clone)
            if match[2].startswith('"'):
                name = '"' + name + '"'
            return match[1] + name
        if match[3]:
            return './' + alias(match[3], clone) + '.nix'
        if match[4]:
            return alias(match[4], clone) + '-module'
        return match[0]

    text = tokens.sub(token, text)
    lines = []
    record = False
    for line in text.splitlines(keepends=True):
        if re.match(r'^nix (?:module|file)\b', line):
            record = True
        def fact(match):
            for prefix, name in ((1, 2), (3, 4), (5, 6)):
                if match[name]:
                    return match[prefix] + alias(match[name], clone)
            return match[0]

        lines.append(line if record else facts.sub(fact, line))
    return ''.join(lines)


def expand(text):
    lines = text.splitlines(keepends=True)
    result = []
    at = 0
    while at < len(lines):
        line = lines[at]
        if re.match(r'^\[.*\] (?:is a Module|supports )', line):
            line = re.sub(r'\b(' + identity + r')\b',
                          lambda match: ', '.join(alias(match[1], clone) for clone in range(1, 22)), line)
            result.append(line)
            at += 1
            continue
        end = at + 1
        if rename(line, 1) != line:
            indent = len(line) - len(line.lstrip())
            while end < len(lines) and lines[end].strip() and len(lines[end]) - len(lines[end].lstrip()) > indent:
                end += 1
            block = ''.join(lines[at:end])
            result.extend(rename(block, clone) for clone in range(1, 22))
        else:
            result.append(line)
        at = end
    return ''.join(result)


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


for clone in range(1, 22):
    for name in names:
        for path in sorted((source / 'modules' / name).glob('*.gal')):
            filename = alias(name, clone) + '.gal' if path.stem == name else path.name
            write(out / 'modules' / alias(name, clone) / filename, rename(path.read_text(), clone))
for path in sorted(source.rglob('*.gal')):
    relative = path.relative_to(source)
    if relative.parts[0] != 'modules':
        write(out / relative, expand(path.read_text()))
print('generated one Galileo program: 21 clones, 210 modules')
