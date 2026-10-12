package schema

import "list"

review: {
	I1_enabled_tag: {
		for module in _catalog
		if list.Contains(export.enabled, module.name) && !list.Contains(_selection.direct, module.name) {
			(module.name): true & (len([
				for selection in _selection.selected
				if (list.Contains(module.tags, selection.tag) && !list.Contains(selection.exclude, module.name)) || (list.Contains(selection.optIn, module.name) && list.Contains(module.optIn, selection.tag)) {selection.tag},
			]) > 0)
		}
	}
	I2_supported_platform: {
		for name in export.enabled {
			(name): true & (_selection.platform == "linux" || list.Contains(_allowed, name))
		}
	}
	I3_tag_override: {
		for name, settings in export.settings.myConfig.modules
		for declaredName, tags in _tagOverrides if name == declaredName
		for tag, values in tags
		for option, value in values
		for appliedOption, appliedValue in settings if option == appliedOption {
			(name): (tag): (option): true & (_tagActive[name] && list.Contains(_contributions[name], tag))
		}
	}
}
