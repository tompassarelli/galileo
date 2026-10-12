_review_tag_contributes = lambda name, tag {
    tag in _tags and name in _metadata and name not in (_minus[tag] if tag in _minus else []) and (tag in _metadata[name].tags or (tag in _metadata[name].optins and name in (_plus[tag] if tag in _plus else [])))
}
_review_assignments = []
assert all name in enabled {name in _direct or any tag in _tags {_review_tag_contributes(name, tag)}}, "I1 rule.tag-resolution: enabled module requires a selected contributing tag or direct host enable"
assert all name in enabled {_linux or name in _darwin}, "I2 rule.platform-gate: every enabled module requires host platform support"
assert _review_tag_contributes("firefox", "browsers") if "firefox" in settings.myConfig.modules and "default" in settings.myConfig.modules.firefox, "I3 rule.tag-override: Firefox default override requires browsers to contribute Firefox"
assert all module in _review_assignments {module in enabled}, "I4 rule.realization: assigned realization requires an enabled module"
