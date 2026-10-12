package schema

import "list"

#Metadata: {
	name: string
	tags: [...string]
	optIn: [...string]
}
context: {[string]: {tags: [...string], optins: [...string]}}
_context: [for name, metadata in context {name: name, tags: metadata.tags, optIn: metadata.optins}]
_sampleOrder: ["firefox", "zen-browser", "kanata", "users", "lane-gc", "atuin", "chrome", "godot", "bun", "obsidian"]
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
_tagActive: {
	for module in _catalog {
		(module.name): len(_contributions[module.name]) > 0 && !list.Contains(_selection.disabled, module.name) && (_selection.platform == "linux" || list.Contains(_allowed, module.name))
	}
}
_active: {
	for module in _catalog {
		(module.name): (_tagActive[module.name] || list.Contains(_selection.direct, module.name)) && !list.Contains(_selection.disabled, module.name) && (_selection.platform == "linux" || list.Contains(_allowed, module.name))
	}
}
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
_firefoxDefault: *true | bool
_firefoxDefault: _tagOverrides.firefox.browsers.default

_requirements: *[] | [...string]
_realizers: *{} | {[string]: [...string]}
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
