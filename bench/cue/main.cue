package schema

_host: "whiterabbit" | "ashashi" @tag(host)

export: {
	enabled: [for module in _sample if _active[module.name] {module.name}]
	settings: myConfig: modules: _settings
	modules: {
		for module in _sample if _host == "whiterabbit" || _active[module.name] {
			(module.name): _bodies[module.name]
		}
	}
}

private: {
	whiterabbit: {
		users: email:    #Options["myConfig.modules.users.email"]
		kanata: devices: #Options["myConfig.modules.kanata.devices"]
	}
	ashashi: users: email: #Options["myConfig.modules.users.email"]
}
