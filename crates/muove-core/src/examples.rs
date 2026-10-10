use crate::*;

fn slots(spec: &[(&str, &str)]) -> Vec<Slot> {
    spec.iter()
        .map(|(name, ty)| Slot::new(*name, Type::entity(*ty)))
        .collect()
}
fn vars(names: &[&str]) -> Vec<Expr> {
    names.iter().map(|name| Expr::var(*name)).collect()
}
fn atom(id: RelationId, names: &[&str]) -> Atom {
    Atom::new(id, vars(names))
}
fn positive(id: RelationId, names: &[&str]) -> Literal {
    atom(id, names).positive()
}
fn negative(id: RelationId, names: &[&str]) -> Literal {
    atom(id, names).negative()
}
fn named_fact(id: RelationId, arguments: &[(&str, &str)]) -> Fact {
    Fact::new(
        id,
        arguments.iter().map(|(ty, name)| Value::entity(*ty, *name)),
    )
}
fn shape(
    p: &mut ProgramBuilder,
    name: &str,
    kind: RelationKind,
    spec: &[(&str, &str)],
    template: &str,
) -> RelationId {
    p.relation(Relation::new(name, kind, slots(spec)).template(template))
}
fn add_rule(
    p: &mut ProgramBuilder,
    name: &str,
    head: RelationId,
    names: &[&str],
    body: Vec<Literal>,
) {
    p.rule(Rule::new(name, atom(head, names)).when(body));
}

pub fn dungeon_static() -> Program {
    let mut p = Program::builder();
    for ty in ["Hero", "Room", "Door", "Key", "Trap"] {
        p.entity_type(ty);
    }
    for (ty, names) in [
        ("Hero", vec!["Ash"]),
        (
            "Room",
            vec![
                "Entrance", "Hall", "Library", "Armory", "Crypt", "Vault", "Shrine",
            ],
        ),
        (
            "Door",
            vec![
                "Front-Door",
                "Library-Arch",
                "Armory-Gate",
                "Crypt-Stair",
                "Vault-Gate",
                "Crypt-Door",
                "Shrine-Door",
            ],
        ),
        ("Key", vec!["Bronze-Key", "Iron-Key", "Silver-Key"]),
        ("Trap", vec!["Spike-Trap"]),
    ] {
        for name in names {
            p.entity(Entity::named(ty, name));
        }
    }
    let starts = shape(
        &mut p,
        "level/starts-in",
        RelationKind::Static,
        &[("hero", "Hero"), ("room", "Room")],
        "{hero} starts in {room}",
    );
    let leads = shape(
        &mut p,
        "level/leads",
        RelationKind::Static,
        &[("door", "Door"), ("here", "Room"), ("there", "Room")],
        "{door} leads from {here} to {there}",
    );
    let unlocked = shape(
        &mut p,
        "level/unlocked",
        RelationKind::Static,
        &[("door", "Door")],
        "{door} is unlocked",
    );
    let locked = p.relation(
        Relation::new(
            "level/locked-by",
            RelationKind::Static,
            slots(&[("door", "Door"), ("key", "Key")]),
        )
        .template("{door} is locked by {key}")
        .mode(Mode::AtMostOne {
            keys: vec![0],
            values: vec![1],
        }),
    );
    let lies = shape(
        &mut p,
        "level/lies-in",
        RelationKind::Static,
        &[("key", "Key"), ("room", "Room")],
        "{key} lies in {room}",
    );
    let reach = p.relation(
        Relation::new(
            "level/reach",
            RelationKind::Derived,
            slots(&[("hero", "Hero"), ("room", "Room")]),
        )
        .template("{hero} can reach {room}")
        .checkpoint(),
    );
    let holds = shape(
        &mut p,
        "level/holds",
        RelationKind::Derived,
        &[("hero", "Hero"), ("key", "Key")],
        "{hero} holds {key}",
    );
    let armed = shape(
        &mut p,
        "level/armed-in",
        RelationKind::Static,
        &[("trap", "Trap"), ("room", "Room")],
        "{trap} is armed in {room}",
    );
    let telegraphed = shape(
        &mut p,
        "level/telegraphed",
        RelationKind::Static,
        &[("trap", "Trap")],
        "{trap} is telegraphed",
    );
    let hurts = shape(
        &mut p,
        "level/hurts",
        RelationKind::Derived,
        &[("trap", "Trap"), ("hero", "Hero")],
        "{trap} hurts {hero}",
    );
    p.fact(named_fact(starts, &[("Hero", "Ash"), ("Room", "Entrance")]));
    for (door, here, there, key) in [
        ("Front-Door", "Entrance", "Hall", None),
        ("Library-Arch", "Hall", "Library", None),
        ("Armory-Gate", "Hall", "Armory", Some("Bronze-Key")),
        ("Crypt-Stair", "Library", "Crypt", None),
        ("Vault-Gate", "Armory", "Vault", Some("Bronze-Key")),
        ("Crypt-Door", "Crypt", "Vault", Some("Iron-Key")),
        ("Shrine-Door", "Crypt", "Shrine", Some("Silver-Key")),
    ] {
        p.fact(named_fact(
            leads,
            &[("Door", door), ("Room", here), ("Room", there)],
        ));
        if let Some(key) = key {
            p.fact(named_fact(locked, &[("Door", door), ("Key", key)]));
        } else {
            p.fact(named_fact(unlocked, &[("Door", door)]));
        }
    }
    p.fact(named_fact(
        lies,
        &[("Key", "Bronze-Key"), ("Room", "Library")],
    ))
    .fact(named_fact(lies, &[("Key", "Iron-Key"), ("Room", "Crypt")]))
    .fact(named_fact(
        armed,
        &[("Trap", "Spike-Trap"), ("Room", "Hall")],
    ))
    .fact(named_fact(telegraphed, &[("Trap", "Spike-Trap")]));
    add_rule(
        &mut p,
        "level/start",
        reach,
        &["hero", "room"],
        vec![positive(starts, &["hero", "room"])],
    );
    add_rule(
        &mut p,
        "level/walk",
        reach,
        &["hero", "there"],
        vec![
            positive(reach, &["hero", "here"]),
            positive(leads, &["door", "here", "there"]),
            positive(unlocked, &["door"]),
        ],
    );
    add_rule(
        &mut p,
        "level/unlock",
        reach,
        &["hero", "there"],
        vec![
            positive(reach, &["hero", "here"]),
            positive(leads, &["door", "here", "there"]),
            positive(locked, &["door", "key"]),
            positive(holds, &["hero", "key"]),
        ],
    );
    add_rule(
        &mut p,
        "level/pick-up",
        holds,
        &["hero", "key"],
        vec![
            positive(lies, &["key", "room"]),
            positive(reach, &["hero", "room"]),
        ],
    );
    add_rule(
        &mut p,
        "level/trap-strike",
        hurts,
        &["trap", "hero"],
        vec![
            positive(armed, &["trap", "room"]),
            positive(telegraphed, &["trap"]),
            positive(reach, &["hero", "room"]),
        ],
    );
    let vault = named_fact(reach, &[("Hero", "Ash"), ("Room", "Vault")]).atom();
    let library = named_fact(reach, &[("Hero", "Ash"), ("Room", "Library")]).atom();
    p.requirement(Requirement::new(
        "reach-vault",
        Claim::State(StateClaim::Holds(vault.clone())),
    ))
    .requirement(Requirement::new(
        "library-required",
        Claim::Requires {
            target: vault.clone(),
            condition: library,
        },
    ))
    .requirement(Requirement::new(
        "no-key-failure",
        Claim::NoSinglePointOfFailure {
            claim: Box::new(Claim::State(StateClaim::Holds(vault))),
            among: atom(lies, &["key", "room"]),
        },
    ))
    .requirement(Requirement::new(
        "trap-telegraphed",
        Claim::Requires {
            target: atom(hurts, &["trap", "hero"]),
            condition: atom(telegraphed, &["trap"]),
        },
    ))
    .requirement(Requirement::new(
        "never-shrine",
        Claim::Never(StateClaim::Holds(
            named_fact(reach, &[("Hero", "Ash"), ("Room", "Shrine")]).atom(),
        )),
    ));
    p.query(
        "reachable-rooms",
        Query::all(
            [Variable::new("room")],
            [Atom::new(
                reach,
                [Expr::value(Value::entity("Hero", "Ash")), Expr::var("room")],
            )
            .positive()],
        ),
    );
    p.build()
}

pub fn tags() -> Program {
    let mut p = Program::builder();
    for ty in ["Module", "Tag", "Host", "Platform"] {
        p.entity_type(ty);
    }
    let tagged = shape(
        &mut p,
        "firn/is-tagged",
        RelationKind::Static,
        &[("module", "Module"), ("tag", "Tag")],
        "{module} is tagged {tag}",
    );
    let opt = shape(
        &mut p,
        "firn/can-opt-into",
        RelationKind::Static,
        &[("module", "Module"), ("tag", "Tag")],
        "{module} can opt into {tag}",
    );
    let enables = shape(
        &mut p,
        "firn/enables",
        RelationKind::Static,
        &[("host", "Host"), ("tag", "Tag")],
        "{host} enables {tag}",
    );
    let drops = shape(
        &mut p,
        "firn/drops-from",
        RelationKind::Static,
        &[("host", "Host"), ("module", "Module"), ("tag", "Tag")],
        "{host} drops {module} from {tag}",
    );
    let adds = shape(
        &mut p,
        "firn/adds-to",
        RelationKind::Static,
        &[("host", "Host"), ("module", "Module"), ("tag", "Tag")],
        "{host} adds {module} to {tag}",
    );
    let disables = shape(
        &mut p,
        "firn/disables",
        RelationKind::Static,
        &[("host", "Host"), ("module", "Module")],
        "{host} disables {module}",
    );
    let runs = p.relation(
        Relation::new(
            "firn/runs",
            RelationKind::Static,
            slots(&[("host", "Host"), ("platform", "Platform")]),
        )
        .template("{host} runs {platform}")
        .mode(Mode::ExactlyOne {
            key: 0,
            values: vec![1],
        }),
    );
    let supports = shape(
        &mut p,
        "firn/supports",
        RelationKind::Static,
        &[("module", "Module"), ("platform", "Platform")],
        "{module} supports {platform}",
    );
    let sets = p.relation(
        Relation::new(
            "firn/sets-for",
            RelationKind::Static,
            [
                Slot::new("tag", Type::entity("Tag")),
                Slot::new("path", Type::OptionPath),
                Slot::new("value", Type::Any.nix()),
                Slot::new("module", Type::entity("Module")),
            ],
        )
        .template("{tag} sets {path} to {value} for {module}"),
    );
    let by = shape(
        &mut p,
        "firn/enabled-by",
        RelationKind::Derived,
        &[("module", "Module"), ("host", "Host"), ("tag", "Tag")],
        "{module} is enabled on {host} by {tag}",
    );
    let default = shape(
        &mut p,
        "firn/enabled-by-default",
        RelationKind::Derived,
        &[("module", "Module"), ("host", "Host"), ("tag", "Tag")],
        "{module} is enabled on {host} by default through {tag}",
    );
    let enabled = p.relation(
        Relation::new(
            "firn/enabled",
            RelationKind::Derived,
            slots(&[("module", "Module"), ("host", "Host")]),
        )
        .template("{module} is enabled on {host}")
        .checkpoint(),
    );
    let gets = p.relation(
        Relation::new(
            "firn/gets-set-by",
            RelationKind::Derived,
            [
                Slot::new("host", Type::entity("Host")),
                Slot::new("path", Type::OptionPath),
                Slot::new("value", Type::Any.nix()),
                Slot::new("module", Type::entity("Module")),
            ],
        )
        .template("{host} gets {path} set to {value} by {module}")
        .mode(Mode::AtMostOne {
            keys: vec![0, 1],
            values: vec![2],
        }),
    );
    add_rule(
        &mut p,
        "enabled-by-default",
        default,
        &["module", "host", "tag"],
        vec![
            positive(enables, &["host", "tag"]),
            positive(tagged, &["module", "tag"]),
            negative(drops, &["host", "module", "tag"]),
        ],
    );
    add_rule(
        &mut p,
        "default-membership-enables",
        by,
        &["module", "host", "tag"],
        vec![positive(default, &["module", "host", "tag"])],
    );
    add_rule(
        &mut p,
        "enabled-by-opt-in",
        by,
        &["module", "host", "tag"],
        vec![
            positive(enables, &["host", "tag"]),
            positive(adds, &["host", "module", "tag"]),
            positive(opt, &["module", "tag"]),
        ],
    );
    add_rule(
        &mut p,
        "enabled",
        enabled,
        &["module", "host"],
        vec![
            positive(by, &["module", "host", "tag"]),
            negative(disables, &["host", "module"]),
        ],
    );
    add_rule(
        &mut p,
        "tag-overrides",
        gets,
        &["host", "path", "value", "module"],
        vec![
            positive(enabled, &["module", "host"]),
            positive(default, &["module", "host", "tag"]),
            positive(sets, &["tag", "path", "value", "module"]),
        ],
    );
    p.entity(Entity::named("Host", "whiterabbit"))
        .entity(Entity::named("Platform", "linux"));
    p.fact(named_fact(
        runs,
        &[("Host", "whiterabbit"), ("Platform", "linux")],
    ));
    for tag in [
        "terminal",
        "cli-tools",
        "theming",
        "auth",
        "javascript",
        "python",
        "rust",
        "browsers",
        "desktop",
    ] {
        p.entity(Entity::named("Tag", tag)).fact(named_fact(
            enables,
            &[("Host", "whiterabbit"), ("Tag", tag)],
        ));
    }
    for module in [
        "firefox",
        "chrome",
        "qutebrowser",
        "zen-browser",
        "mako",
        "river",
        "anytype",
        "bun",
        "cargo",
        "clang",
    ] {
        p.entity(Entity::named("Module", module));
    }
    p.fact(named_fact(
        tagged,
        &[("Module", "firefox"), ("Tag", "browsers")],
    ))
    .fact(named_fact(
        drops,
        &[
            ("Host", "whiterabbit"),
            ("Module", "mako"),
            ("Tag", "desktop"),
        ],
    ));
    for (module, tag) in [
        ("chrome", "browsers"),
        ("qutebrowser", "browsers"),
        ("zen-browser", "browsers"),
        ("river", "desktop"),
    ] {
        p.fact(named_fact(
            adds,
            &[("Host", "whiterabbit"), ("Module", module), ("Tag", tag)],
        ));
    }
    for module in ["anytype", "bun", "cargo", "clang"] {
        p.fact(named_fact(
            disables,
            &[("Host", "whiterabbit"), ("Module", module)],
        ));
    }
    p.fact(Fact::new(
        sets,
        [
            Value::entity("Tag", "browsers"),
            Value::OptionPath(vec![
                "myConfig".into(),
                "modules".into(),
                "firefox".into(),
                "default".into(),
            ]),
            Value::Nix {
                normalized: "true".into(),
            },
            Value::entity("Module", "firefox"),
        ],
    ));
    p.requirement(Requirement::new(
        "opt-in-membership",
        Claim::State(StateClaim::Every {
            variables: ["host", "module", "tag"].map(Variable::new).to_vec(),
            domain: vec![positive(adds, &["host", "module", "tag"])],
            body: vec![positive(opt, &["module", "tag"])],
        }),
    ))
    .requirement(Requirement::new(
        "platform-supported",
        Claim::State(StateClaim::Every {
            variables: ["module", "host", "platform"].map(Variable::new).to_vec(),
            domain: vec![
                positive(enabled, &["module", "host"]),
                positive(runs, &["host", "platform"]),
            ],
            body: vec![positive(supports, &["module", "platform"])],
        }),
    ));
    p.query(
        "enabled-on-whiterabbit",
        Query::all(
            [Variable::new("module")],
            [Atom::new(
                enabled,
                [
                    Expr::var("module"),
                    Expr::value(Value::entity("Host", "whiterabbit")),
                ],
            )
            .positive()],
        ),
    );
    p.build()
}

pub fn r13() -> Program {
    let mut p = Program::builder();
    let left = p.relation(Relation::new(
        "r13/left",
        RelationKind::Static,
        [Slot::new("stage", Type::Int)],
    ));
    let right = p.relation(Relation::new(
        "r13/right",
        RelationKind::Static,
        [Slot::new("stage", Type::Int)],
    ));
    let stage = p.relation(Relation::new(
        "r13/stage",
        RelationKind::Derived,
        [Slot::new("stage", Type::Int)],
    ));
    let goal = p.relation(Relation::new("r13/goal", RelationKind::Derived, []).checkpoint());
    for index in 0..20 {
        p.fact(Fact::new(left, [Value::Int(index)]))
            .fact(Fact::new(right, [Value::Int(index)]));
    }
    add_rule(
        &mut p,
        "r13/left-stage",
        stage,
        &["stage"],
        vec![positive(left, &["stage"])],
    );
    add_rule(
        &mut p,
        "r13/right-stage",
        stage,
        &["stage"],
        vec![positive(right, &["stage"])],
    );
    p.rule(
        Rule::new("r13/all-stages", Atom::new(goal, []))
            .when((0..20).map(|index| Atom::new(stage, [Expr::value(index)]).positive())),
    );
    p.requirement(Requirement::new(
        "r13/goal",
        Claim::State(StateClaim::Holds(Atom::new(goal, []))),
    ));
    p.build()
}

fn gate_program() -> (ProgramBuilder, RelationId, RelationId) {
    let mut p = Program::builder();
    p.entity_type("Node");
    for name in ["Seed", "Gate", "Goal"] {
        p.entity(Entity::named("Node", name));
    }
    let node = shape(&mut p, "w/node", RelationKind::Static, &[("node", "Node")], "{node} is a node");
    let holds = shape(&mut p, "w/holds", RelationKind::Derived, &[("node", "Node")], "{node} holds");
    let fact = |id, name| named_fact(id, &[("Node", name)]);
    for name in ["Seed", "Gate", "Goal"] {
        p.fact(fact(node, name));
    }
    p.fact(fact(holds, "Seed"));
    for (name, head, body) in [("w/make-gate", "Gate", "Seed"), ("w/make-goal", "Goal", "Gate")] {
        p.rule(Rule::new(name, fact(holds, head).atom()).when([fact(holds, body).atom().positive()]));
    }
    p.requirement(Requirement::new(
        "goal-holds",
        Claim::State(StateClaim::Holds(fact(holds, "Goal").atom())),
    ))
    .requirement(Requirement::new(
        "every-node-holds",
        Claim::State(StateClaim::Every {
            variables: vec![Variable::new("n")],
            domain: vec![positive(node, &["n"])],
            body: vec![positive(holds, &["n"])],
        }),
    ));
    (p, node, holds)
}

pub fn gate() -> Program {
    gate_program().0.build()
}

pub fn gate_never() -> Program {
    let (mut p, _, holds) = gate_program();
    p.requirement(Requirement::new(
        "never-goal",
        Claim::Never(StateClaim::Holds(
            named_fact(holds, &[("Node", "Goal")]).atom(),
        )),
    ));
    p.build()
}

pub fn chain(rooms: usize) -> Program {
    let mut p = Program::builder();
    p.entity_type("Room");
    let room = |index: usize| ("Room", format!("Room-{index}"));
    for index in 0..rooms {
        p.entity(Entity::named("Room", room(index).1));
    }
    let door = shape(&mut p, "chain/door", RelationKind::Static, &[("here", "Room"), ("there", "Room")], "{here} opens to {there}");
    let reach = shape(&mut p, "chain/reach", RelationKind::Derived, &[("here", "Room"), ("there", "Room")], "{here} reaches {there}");
    for index in 1..rooms {
        let (here, there) = (room(index - 1), room(index));
        p.fact(named_fact(door, &[(here.0, &here.1), (there.0, &there.1)]));
    }
    add_rule(&mut p, "chain/step", reach, &["here", "there"], vec![positive(door, &["here", "there"])]);
    add_rule(
        &mut p,
        "chain/walk",
        reach,
        &["here", "there"],
        vec![positive(reach, &["here", "middle"]), positive(door, &["middle", "there"])],
    );
    let (first, last) = (room(0), room(rooms - 1));
    p.requirement(Requirement::new(
        "first-reaches-last",
        Claim::State(StateClaim::Holds(
            named_fact(reach, &[(first.0, &first.1), (last.0, &last.1)]).atom(),
        )),
    ));
    p.build()
}

pub fn shortcut() -> Program {
    let mut p = Program::builder();
    p.entity_type("Node");
    let names: Vec<String> = ["Seed", "Gate", "Goal"]
        .into_iter()
        .map(String::from)
        .chain((1..=5).map(|i| format!("C{i}")))
        .collect();
    for name in &names {
        p.entity(Entity::named("Node", name.as_str()));
    }
    let holds = shape(&mut p, "w/holds", RelationKind::Derived, &[("node", "Node")], "{node} holds");
    let fact = |name: &str| named_fact(holds, &[("Node", name)]);
    p.fact(fact("Seed")).fact(fact("Gate"));
    for i in 1..=5 {
        let head = format!("C{i}");
        let previous = if i == 1 { "Seed".to_string() } else { format!("C{}", i - 1) };
        for (rule, body) in [(format!("w/shortcut-{i}"), "Gate"), (format!("w/chain-{i}"), previous.as_str())] {
            p.rule(Rule::new(rule, fact(&head).atom()).when([fact(body).atom().positive()]));
        }
    }
    p.rule(Rule::new("w/finish", fact("Goal").atom()).when([fact("C5").atom().positive()]));
    p.requirement(Requirement::new("goal-holds", Claim::State(StateClaim::Holds(fact("Goal").atom()))))
        .requirement(Requirement::new(
            "goal-requires-gate",
            Claim::Requires {
                target: fact("Goal").atom(),
                condition: fact("Gate").atom(),
            },
        ))
        .requirement(Requirement::new(
            "goal-survives-gate",
            Claim::NoSinglePointOfFailure {
                claim: Box::new(Claim::State(StateClaim::Holds(fact("Goal").atom()))),
                among: fact("Gate").atom(),
            },
        ));
    p.build()
}
