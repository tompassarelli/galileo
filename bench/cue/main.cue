package schema

_host: "whiterabbit" | "ashashi" @tag(host)

export: {
	enabled: [for name in _part1 if _active[name] {name}]
	settings: myConfig: modules: _settings
	modules: {
		for name in _part1 if _host == "whiterabbit" {
			(name): _bodies[name]
		}
	}
}

_part1: ["firefox", "zen-browser", "kanata", "users", "lane-gc"]

private: {
	whiterabbit: {
		users: email:    #Options["myConfig.modules.users.email"]
		kanata: devices: #Options["myConfig.modules.kanata.devices"]
	}
	ashashi: users: email: #Options["myConfig.modules.users.email"]
}
