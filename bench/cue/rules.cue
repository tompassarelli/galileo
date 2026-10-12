package schema

import (
	"list"
	"strings"
)

#Metadata: {
	name: string
	tags: [...string]
	optIn: [...string]
}
context: {[string]: {tags: [...string], optins: [...string]}}
_context: [for module, metadata in context {name: module, tags: metadata.tags, optIn: metadata.optins}]
_sample: [
	{name: "firefox", tags: ["browsers"], optIn: []},
	{name: "zen-browser", tags: [], optIn: ["browsers"]},
	{name: "kanata", tags: [], optIn: []},
	{name: "users", tags: [], optIn: []},
	{name: "lane-gc", tags: [], optIn: []},
	{name: "atuin", tags: ["terminal"], optIn: []},
	{name: "chrome", tags: [], optIn: ["browsers"]},
	{name: "godot", tags: ["creative"], optIn: []},
	{name: "bun", tags: ["javascript"], optIn: []},
	{name: "obsidian", tags: ["notes"], optIn: []},
]
_catalog: list.Concat([_sample, _context]) & [...#Metadata]
_allowed: ["atuin", "bash", "bc", "beagle", "btop", "curl", "delta", "direnv", "dust", "eza", "fastfetch", "fd", "forgejo-cli", "gh", "ghostscript", "ghostty", "git", "gitleaks", "glow", "imagemagick", "jq", "kitty", "procs", "promptfoo", "ripgrep", "starship", "tealdeer", "tree", "unrar", "unzip", "vim", "wget", "yazi", "zoxide"]
_contributions: {
	for module in _catalog {
		(module.name): [
			for selection in _selection.selected
			if (list.Contains(module.tags, selection.tag) && !list.Contains(selection.exclude, module.name)) || (list.Contains(selection.optIn, module.name) && list.Contains(module.optIn, selection.tag)) {selection.tag},
		]
	}
}
_eligible: {for m in _catalog {(m.name): !list.Contains(_selection.disabled, m.name) && (_selection.platform == "linux" || list.Contains(_allowed, m.name))}}
_tagActive: {for m in _catalog {(m.name): _eligible[m.name] && len(_contributions[m.name]) > 0}}
_active: {for m in _catalog {(m.name): _eligible[m.name] && (_tagActive[m.name] || list.Contains(_selection.direct, m.name))}}
_tagOverrides: firefox: browsers: default: true
_overrides: {
	for name, tags in _tagOverrides
	for tag, values in tags
	if _tagActive[name] && list.Contains(_contributions[name], tag) {
		(name): {
			for option, value in values {
				(option): {"$call": {function: {"$ref": ["lib", "mkDefault"]}, args: [value]}}
			}
		}
	}
}
_firefoxDefault: _tagOverrides.firefox.browsers.default

_requirements: [for fact in _responsibilities if fact[1] == "requires_software" {fact[2]}]
_realizers: {
	for fact in _responsibilities if fact[1] == "is_realized_by" {
		(fact[0]): [strings.TrimSuffix(fact[2], "-module")]
	}
}
_realization: {
	for software in _requirements {
		(software): true & list.Contains([for name in _realizers[software] if _active[name] {true}], true)
	}
}
_inputDeclarations: [
	{module: "firefox", name: "nur", url: "github:nix-community/NUR"},
	{module: "zen-browser", name: "zen-browser", url: "github:0xc000022070/zen-browser-flake"},
]
_inputs: {
	for input in _inputDeclarations if _active[input.module] {(input.name): input.url}
}
