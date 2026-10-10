//! Elaboration: indentation blocks -> a checked [`Program`].
//!
//! The surface is the original Clause one: bare type names, relation
//! sentence shapes, models of facts, derivation rules, revisions, and
//! questions. Every sentence is read against the declared shapes, with names
//! and variables checked against the role types.

use crate::syntax::{Block, Error, Line, blocks};
use std::collections::{BTreeSet, HashMap};

/// A ground relational fact: one relation and one entity per role.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fact {
    pub rel: usize,
    pub args: Vec<usize>,
}

#[derive(Clone, Debug)]
pub enum Piece {
    Word(String),
    Slot(usize),
}

#[derive(Clone, Debug)]
pub struct Role {
    pub name: String,
    pub ty: usize,
}

#[derive(Clone, Debug)]
pub struct Relation {
    pub name: String,
    pub roles: Vec<Role>,
    pub template: Vec<Piece>,
    /// Declared computation directions. Recorded and checked, not yet used
    /// for planning: evaluation is bottom-up.
    pub modes: Vec<String>,
    /// True when some rule concludes this relation.
    pub derived: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Term {
    Var(usize),
    Const(usize),
}

#[derive(Clone, Debug)]
pub struct Atom {
    pub rel: usize,
    pub terms: Vec<Term>,
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub name: String,
    pub model: usize,
    pub head: Atom,
    pub body: Vec<Atom>,
    pub vars: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Entity {
    pub name: String,
    pub ty: usize,
    pub model: usize,
}

/// A model or a revision of one: a set of asserted facts over visible entities.
#[derive(Clone, Debug)]
pub struct World {
    pub name: String,
    pub model: usize,
    pub base: Option<usize>,
    pub facts: BTreeSet<Fact>,
    pub entities: BTreeSet<usize>,
    /// The rules in force in this world. Revisions can add and remove rules.
    pub rules: BTreeSet<usize>,
    /// What the author of a revision says it does. Shown, never trusted.
    pub says: Option<String>,
    pub line: usize,
}

/// A must-sentence: something every world of a model is checked against.
#[derive(Clone, Debug)]
pub enum IntentKind {
    /// `<sentence>`: it must hold (with variables: at least one match).
    Holds(Atom),
    /// `never <sentence>`: it must not hold.
    Never(Atom),
    /// `<sentence> requires <sentence>`: every way the first holds goes
    /// through something matching the second.
    Requires(Atom, Atom),
    /// `<sentence> has no single point of failure among <pattern>`: no one
    /// stated fact matching the pattern stops it on its own.
    NoSinglePoint(Atom, Atom),
}

#[derive(Clone, Debug)]
pub struct Intent {
    pub model: usize,
    pub line: usize,
    /// The sentence as written.
    pub text: String,
    pub vars: Vec<String>,
    pub kind: IntentKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    All,
    One,
}

#[derive(Clone, Debug)]
pub enum Request {
    Find {
        world: usize,
        var: usize,
        pattern: Atom,
        vars: Vec<String>,
    },
    Why {
        world: usize,
        target: Fact,
        pick: Pick,
    },
    Prevent {
        world: usize,
        target: Fact,
        using: Vec<usize>,
        pick: Pick,
    },
    Achieve {
        world: usize,
        target: Fact,
        using: Vec<usize>,
        pick: Pick,
    },
    Diff {
        base: usize,
        successor: usize,
    },
    WhyNot {
        world: usize,
        target: Fact,
    },
    Review {
        base: usize,
        successor: usize,
    },
}

#[derive(Clone, Debug)]
pub struct RequestItem {
    pub line: usize,
    /// The question as written, for echoing in output.
    pub source: Vec<String>,
    pub request: Request,
}

#[derive(Debug, Default)]
pub struct Program {
    pub types: Vec<String>,
    pub relations: Vec<Relation>,
    pub entities: Vec<Entity>,
    pub models: Vec<String>,
    pub worlds: Vec<World>,
    pub rules: Vec<Rule>,
    pub intents: Vec<Intent>,
    pub requests: Vec<RequestItem>,
    type_index: HashMap<String, usize>,
    relation_index: HashMap<String, usize>,
    entity_index: HashMap<(usize, String), usize>,
    world_index: HashMap<String, usize>,
}

enum Kind {
    Type,
    Relation(String),
    Rule(String),
    Revision(String),
    Intents(String),
    Model,
    Request,
    Using,
}

enum Item {
    Member { line: Line, name: String, ty: usize },
    Fact { line: Line, text: String },
}

enum Filler {
    Var(String, usize),
    Const(usize),
}

#[derive(Default)]
struct Vars {
    names: Vec<String>,
    types: Vec<usize>,
}

impl Vars {
    fn type_of(&self, name: &str) -> Option<usize> {
        self.names
            .iter()
            .position(|n| n == name)
            .map(|i| self.types[i])
    }

    fn index(&mut self, name: &str, ty: usize) -> usize {
        match self.names.iter().position(|n| n == name) {
            Some(i) => i,
            None => {
                self.names.push(name.to_string());
                self.types.push(ty);
                self.names.len() - 1
            }
        }
    }
}

fn classify(block: &Block) -> Kind {
    let text = block.line.text.as_str();
    if is_request_header(text) {
        Kind::Request
    } else if text == "using:" {
        Kind::Using
    } else if let Some(name) = text.strip_suffix(": RelationShape") {
        Kind::Relation(name.trim().to_string())
    } else if let Some(name) = text.strip_suffix(": DerivationRule") {
        Kind::Rule(name.trim().to_string())
    } else if let Some(name) = text.strip_suffix(": Revision") {
        Kind::Revision(name.trim().to_string())
    } else if let Some(model) = text.strip_suffix(" must:").filter(|_| !block.children.is_empty()) {
        Kind::Intents(model.trim().to_string())
    } else if block.children.is_empty() {
        Kind::Type
    } else {
        Kind::Model
    }
}

fn is_request_header(text: &str) -> bool {
    text.starts_with("find all ?")
        || text.starts_with("why all in ")
        || text.starts_with("why in ")
        || text.starts_with("why not in ")
        || (text.starts_with("review ") && text.contains(" -> "))
        || ((text.starts_with("prevent ") || text.starts_with("achieve "))
            && text.contains(" minimal in "))
        || (text.starts_with("diff ") && text.contains(" -> "))
}

fn is_name(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '/' | '.'))
}

fn is_identifier(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
}

fn is_entity_name(text: &str) -> bool {
    !text.is_empty()
        && !text.starts_with('?')
        && !text.chars().any(|c| "{}[]∈:".contains(c))
}

fn strip_brackets(text: &str) -> (&str, bool) {
    match text.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
        Some(inner) => (inner.trim(), true),
        None => (text.trim(), false),
    }
}

fn parse_range(text: &str) -> Option<(i64, i64)> {
    let (a, b) = text.split_once("..")?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

/// All ways to split `tokens` across the template, as token ranges per slot.
fn splits(
    tokens: &[&str],
    ti: usize,
    template: &[Piece],
    pi: usize,
    current: &mut Vec<(usize, usize)>,
    out: &mut Vec<Vec<(usize, usize)>>,
) {
    if pi == template.len() {
        if ti == tokens.len() {
            out.push(current.clone());
        }
        return;
    }
    match &template[pi] {
        Piece::Word(word) => {
            if ti < tokens.len() && tokens[ti] == word {
                splits(tokens, ti + 1, template, pi + 1, current, out);
            }
        }
        Piece::Slot(_) => {
            for end in ti + 1..=tokens.len() {
                current.push((ti, end));
                splits(tokens, end, template, pi + 1, current, out);
                current.pop();
            }
        }
    }
}

impl Program {
    /// Read and check a whole source file.
    pub fn parse(source: &str) -> Result<Program, Error> {
        let top = blocks(source)?;
        let kinds: Vec<Kind> = top.iter().map(classify).collect();
        let mut p = Program::default();

        for (block, kind) in top.iter().zip(&kinds) {
            if let Kind::Type = kind {
                p.declare_type(&block.line)?;
            }
        }
        for (block, kind) in top.iter().zip(&kinds) {
            if let Kind::Relation(name) = kind {
                p.declare_relation(name, block)?;
            }
        }
        let mut model_blocks = Vec::new();
        for (block, kind) in top.iter().zip(&kinds) {
            if let Kind::Model = kind {
                let model = p.declare_model(block)?;
                model_blocks.push((model, block));
            }
        }
        for (model, block) in &model_blocks {
            p.fill_model(*model, block)?;
        }
        for (block, kind) in top.iter().zip(&kinds) {
            if let Kind::Rule(name) = kind {
                let id = p.read_rule(name, &block.line, &block.children)?;
                let root = p.world_index[&p.models[p.rules[id].model]];
                p.worlds[root].rules.insert(id);
            }
        }
        for (block, kind) in top.iter().zip(&kinds) {
            if let Kind::Intents(model) = kind {
                let Some(m) = p.models.iter().position(|name| name == model) else {
                    return Err(Error::at(&block.line, format!("no model named `{model}`")));
                };
                for child in &block.children {
                    if !child.children.is_empty() {
                        return Err(Error::at(&child.children[0].line, "must-sentences don't nest"));
                    }
                    let intent = p.read_intent(m, &child.line)?;
                    p.intents.push(intent);
                }
            }
        }

        let mut revisions: HashMap<String, &Block> = HashMap::new();
        for (block, kind) in top.iter().zip(&kinds) {
            if let Kind::Revision(name) = kind {
                if !is_name(name) {
                    return Err(Error::at(&block.line, "revision names can't contain spaces"));
                }
                if p.world_index.contains_key(name) || revisions.contains_key(name) {
                    return Err(Error::at(&block.line, format!("`{name}` is already defined")));
                }
                revisions.insert(name.clone(), block);
            }
        }
        let mut names: Vec<&String> = revisions.keys().collect();
        names.sort_by_key(|name| revisions[*name].line.no);
        for name in names {
            p.resolve_revision(name, &revisions, &mut Vec::new())?;
        }

        let mut index = 0;
        while index < top.len() {
            match kinds[index] {
                Kind::Request => {
                    let next = top.get(index + 1).filter(|b| b.line.text == "using:");
                    let (item, used_next) = p.read_request(&top[index], next)?;
                    p.requests.push(item);
                    index += if used_next { 2 } else { 1 };
                }
                Kind::Using => {
                    return Err(Error::at(
                        &top[index].line,
                        "`using:` must directly follow a `prevent` or `achieve` question",
                    ));
                }
                _ => index += 1,
            }
        }
        Ok(p)
    }

    pub fn world(&self, name: &str) -> Option<usize> {
        self.world_index.get(name).copied()
    }

    pub fn relation(&self, name: &str) -> Option<usize> {
        self.relation_index.get(name).copied()
    }

    /// Read one ground sentence in the scope of a named world.
    pub fn fact(&self, world: &str, text: &str) -> Result<Fact, Error> {
        let w = self
            .world(world)
            .ok_or_else(|| Error::bare(format!("no model or revision named `{world}`")))?;
        let line = Line {
            no: 0,
            indent: 0,
            text: text.to_string(),
        };
        let world = &self.worlds[w];
        let atom = self.read_atom(&line, text, world.model, Some(&world.entities), None)?;
        Ok(Self::ground(&atom))
    }

    /// Render a fact back through its relation's sentence shape.
    pub fn sentence(&self, fact: &Fact) -> String {
        self.render_template(fact.rel, |slot| self.entities[fact.args[slot]].name.clone())
    }

    /// Render a pattern, showing variables as `?name`.
    pub fn atom_sentence(&self, atom: &Atom, vars: &[String]) -> String {
        self.render_template(atom.rel, |slot| match atom.terms[slot] {
            Term::Var(v) => format!("?{}", vars[v]),
            Term::Const(c) => self.entities[c].name.clone(),
        })
    }

    /// The declared shape, e.g. `{door} connects {origin} to {destination}`.
    pub fn shape(&self, rel: usize) -> String {
        let relation = &self.relations[rel];
        self.render_template(rel, |slot| format!("{{{}}}", relation.roles[slot].name))
    }

    fn render_template(&self, rel: usize, mut fill: impl FnMut(usize) -> String) -> String {
        let mut words = Vec::new();
        for piece in &self.relations[rel].template {
            match piece {
                Piece::Word(word) => words.push(word.clone()),
                Piece::Slot(slot) => words.push(fill(*slot)),
            }
        }
        words.join(" ")
    }

    fn ground(atom: &Atom) -> Fact {
        Fact {
            rel: atom.rel,
            args: atom
                .terms
                .iter()
                .map(|term| match term {
                    Term::Const(c) => *c,
                    Term::Var(_) => unreachable!("ground atoms have no variables"),
                })
                .collect(),
        }
    }

    fn type_named(&self, line: &Line, name: &str) -> Result<usize, Error> {
        self.type_index.get(name).copied().ok_or_else(|| {
            Error::at(
                line,
                format!("unknown type `{name}` (declare it on its own line first)"),
            )
        })
    }

    fn declare_type(&mut self, line: &Line) -> Result<(), Error> {
        let name = line.text.trim();
        if !is_identifier(name) {
            return Err(Error::at(
                line,
                "a line on its own declares a type, and type names are single words",
            ));
        }
        if self.type_index.contains_key(name) {
            return Err(Error::at(line, format!("type `{name}` is declared twice")));
        }
        self.type_index.insert(name.to_string(), self.types.len());
        self.types.push(name.to_string());
        Ok(())
    }

    fn declare_relation(&mut self, name: &str, block: &Block) -> Result<(), Error> {
        if !is_name(name) {
            return Err(Error::at(&block.line, "relation names can't contain spaces"));
        }
        if self.relation_index.contains_key(name) {
            return Err(Error::at(&block.line, format!("relation `{name}` is declared twice")));
        }
        let Some(first) = block.children.first() else {
            return Err(Error::at(
                &block.line,
                "a relation needs its sentence shape on the next line, like `{door: Door} opens {room: Room}`",
            ));
        };
        let (roles, template) = self.parse_template(&first.line)?;
        let mut modes = Vec::new();
        for child in &block.children[1..] {
            let text = child.line.text.as_str();
            let Some(rest) = text.strip_prefix("mode ") else {
                return Err(Error::at(&child.line, "expected a `mode known -> sought: many` line"));
            };
            let (known, sought) = rest
                .split_once("->")
                .ok_or_else(|| Error::at(&child.line, "a mode needs `->`"))?;
            let (sought, cardinality) = sought
                .split_once(':')
                .ok_or_else(|| Error::at(&child.line, "a mode needs `: one` or `: many`"))?;
            if !matches!(cardinality.trim(), "one" | "many" | "maybe" | "some") {
                return Err(Error::at(&child.line, "mode cardinality must be one, maybe, some or many"));
            }
            for role in known.split(',').chain(sought.split(',')) {
                let role = role.trim();
                if !role.is_empty() && !roles.iter().any(|r| r.name == role) {
                    return Err(Error::at(&child.line, format!("`{role}` isn't a role of this relation")));
                }
            }
            modes.push(rest.trim().to_string());
        }
        self.relation_index.insert(name.to_string(), self.relations.len());
        self.relations.push(Relation {
            name: name.to_string(),
            roles,
            template,
            modes,
            derived: false,
        });
        Ok(())
    }

    fn parse_template(&self, line: &Line) -> Result<(Vec<Role>, Vec<Piece>), Error> {
        let mut roles: Vec<Role> = Vec::new();
        let mut pieces = Vec::new();
        let mut rest = line.text.as_str();
        loop {
            rest = rest.trim_start();
            if rest.is_empty() {
                break;
            }
            if let Some(after) = rest.strip_prefix('{') {
                let close = after
                    .find('}')
                    .ok_or_else(|| Error::at(line, "unclosed `{` in sentence shape"))?;
                let (role, ty) = after[..close]
                    .split_once(':')
                    .ok_or_else(|| Error::at(line, "each slot looks like `{role: Type}`"))?;
                let (role, ty) = (role.trim(), ty.trim());
                if !is_identifier(role) {
                    return Err(Error::at(line, format!("`{role}` isn't a valid role name")));
                }
                if roles.iter().any(|r| r.name == role) {
                    return Err(Error::at(line, format!("role `{role}` appears twice")));
                }
                let ty = self.type_named(line, ty)?;
                pieces.push(Piece::Slot(roles.len()));
                roles.push(Role {
                    name: role.to_string(),
                    ty,
                });
                rest = &after[close + 1..];
            } else {
                let end = rest
                    .find(|c: char| c.is_whitespace() || c == '{')
                    .unwrap_or(rest.len());
                let word = &rest[..end];
                if word.contains('}') {
                    return Err(Error::at(line, "unexpected `}` in sentence shape"));
                }
                pieces.push(Piece::Word(word.to_string()));
                rest = &rest[end..];
            }
        }
        if roles.is_empty() {
            return Err(Error::at(line, "a sentence shape needs at least one `{role: Type}` slot"));
        }
        if !pieces.iter().any(|p| matches!(p, Piece::Word(_))) {
            return Err(Error::at(line, "a sentence shape needs at least one word to recognise it by"));
        }
        Ok((roles, pieces))
    }

    fn declare_model(&mut self, block: &Block) -> Result<usize, Error> {
        let name = block.line.text.trim();
        if !is_name(name) {
            return Err(Error::at(
                &block.line,
                "a name followed by indented lines declares a model, and model names are single words",
            ));
        }
        if self.world_index.contains_key(name) {
            return Err(Error::at(&block.line, format!("`{name}` is already defined")));
        }
        let model = self.models.len();
        self.models.push(name.to_string());
        self.world_index.insert(name.to_string(), self.worlds.len());
        self.worlds.push(World {
            name: name.to_string(),
            model,
            base: None,
            facts: BTreeSet::new(),
            entities: BTreeSet::new(),
            rules: BTreeSet::new(),
            says: None,
            line: block.line.no,
        });
        Ok(model)
    }

    fn fill_model(&mut self, model: usize, block: &Block) -> Result<(), Error> {
        let world = self.world_index[&self.models[model]];
        let items = self.items(&block.children)?;
        for item in &items {
            if let Item::Member { line, name, ty } = item {
                let id = self.declare_entity(line, model, name, *ty, true)?;
                self.worlds[world].entities.insert(id);
            }
        }
        for item in &items {
            if let Item::Fact { line, text } = item {
                let visible = self.worlds[world].entities.clone();
                let atom = self.read_atom(line, text, model, Some(&visible), None)?;
                if !self.worlds[world].facts.insert(Self::ground(&atom)) {
                    return Err(Error::at(line, "this fact is stated twice"));
                }
            }
        }
        Ok(())
    }

    fn declare_entity(
        &mut self,
        line: &Line,
        model: usize,
        name: &str,
        ty: usize,
        fresh: bool,
    ) -> Result<usize, Error> {
        if let Some(&id) = self.entity_index.get(&(model, name.to_string())) {
            if fresh {
                return Err(Error::at(line, format!("`{name}` is declared twice")));
            }
            if self.entities[id].ty != ty {
                return Err(Error::at(
                    line,
                    format!("`{name}` is already a {}", self.types[self.entities[id].ty]),
                ));
            }
            return Ok(id);
        }
        let id = self.entities.len();
        self.entities.push(Entity {
            name: name.to_string(),
            ty,
            model,
        });
        self.entity_index.insert((model, name.to_string()), id);
        Ok(id)
    }

    /// Expand member lines, focus blocks and `for` repetitions.
    fn items(&self, children: &[Block]) -> Result<Vec<Item>, Error> {
        let mut items = Vec::new();
        let mut i = 0;
        while i < children.len() {
            let child = &children[i];
            if child.line.text.starts_with("for ") {
                return Err(Error::at(&child.line, "`for` must follow the line or block it repeats"));
            }
            let repeat = children
                .get(i + 1)
                .filter(|next| next.line.text.starts_with("for ") && next.children.is_empty());
            match repeat {
                Some(next) => {
                    let rest = &next.line.text["for ".len()..];
                    let (var, range) = rest
                        .split_once(':')
                        .ok_or_else(|| Error::at(&next.line, "expected `for n: 1..4`"))?;
                    let var = var.trim();
                    let (from, to) = parse_range(range)
                        .ok_or_else(|| Error::at(&next.line, "expected an integer range like `1..4`"))?;
                    if !is_identifier(var) || from > to {
                        return Err(Error::at(&next.line, "expected `for n: 1..4`"));
                    }
                    let placeholder = format!("{{{var}}}");
                    let mentions = child.line.text.contains(&placeholder)
                        || child.children.iter().any(|c| c.line.text.contains(&placeholder));
                    if !mentions {
                        return Err(Error::at(&next.line, format!("nothing above uses `{placeholder}`")));
                    }
                    for n in from..=to {
                        items.extend(self.member_items(child, Some((&placeholder, n.to_string())))?);
                    }
                    i += 2;
                }
                None => {
                    items.extend(self.member_items(child, None)?);
                    i += 1;
                }
            }
        }
        Ok(items)
    }

    fn member_items(&self, block: &Block, subst: Option<(&str, String)>) -> Result<Vec<Item>, Error> {
        let apply = |text: &str| match &subst {
            Some((key, value)) => text.replace(key, value),
            None => text.to_string(),
        };
        let line = &block.line;
        let text = apply(&line.text);
        if text.contains('{') {
            return Err(Error::at(line, "unknown `{placeholder}`; add a `for` line after this"));
        }
        if let Some((lhs, rhs)) = text.split_once('∈') {
            if !block.children.is_empty() {
                return Err(Error::at(line, "a membership line can't have indented lines under it"));
            }
            let ty = self.type_named(line, rhs.trim())?;
            let (inner, bracketed) = strip_brackets(lhs.trim());
            let mut names = Vec::new();
            let last = inner.rsplit(' ').next().unwrap_or("");
            match parse_range(last).filter(|_| bracketed) {
                Some((from, to)) => {
                    let prefix = &inner[..inner.len() - last.len()];
                    if from > to {
                        return Err(Error::at(line, "range runs backwards"));
                    }
                    for n in from..=to {
                        names.push(format!("{prefix}{n}"));
                    }
                }
                None => names.push(inner.to_string()),
            }
            let mut out = Vec::new();
            for name in names {
                if !is_entity_name(&name) {
                    return Err(Error::at(line, format!("`{name}` isn't a valid name")));
                }
                out.push(Item::Member {
                    line: line.clone(),
                    name,
                    ty,
                });
            }
            return Ok(out);
        }
        if block.children.is_empty() {
            return Ok(vec![Item::Fact {
                line: line.clone(),
                text,
            }]);
        }
        // A focus block: the subject begins every sentence underneath it.
        let (subject, _) = strip_brackets(&text);
        let mut out = Vec::new();
        for child in &block.children {
            if !child.children.is_empty() {
                return Err(Error::at(&child.line, "focus blocks only nest one level"));
            }
            out.push(Item::Fact {
                line: child.line.clone(),
                text: format!("{subject} {}", apply(&child.line.text)),
            });
        }
        Ok(out)
    }

    /// Read a rule (`name`, conclusion line, `when:` block) and return its id.
    fn read_rule(&mut self, name: &str, line: &Line, children: &[Block]) -> Result<usize, Error> {
        if self.rules.iter().any(|r| r.name == name) {
            return Err(Error::at(line, format!("a rule named `{name}` already exists")));
        }
        let model_name = name.split_once('/').map(|(m, _)| m).unwrap_or("");
        let model = self
            .models
            .iter()
            .position(|m| m == model_name)
            .ok_or_else(|| {
                Error::at(
                    line,
                    "name rules `<model>/<rule>` so they know which model they belong to",
                )
            })?;
        let [head, when] = children else {
            return Err(Error::at(line, "a rule is one conclusion line, then `when:` with its conditions"));
        };
        if when.line.text != "when:" || when.children.is_empty() || !head.children.is_empty() {
            return Err(Error::at(line, "a rule is one conclusion line, then `when:` with its conditions"));
        }
        let mut vars = Vars::default();
        let mut body = Vec::new();
        for condition in &when.children {
            if !condition.children.is_empty() {
                return Err(Error::at(&condition.line, "conditions don't nest"));
            }
            body.push(self.read_atom(&condition.line, &condition.line.text, model, None, Some(&mut vars))?);
        }
        let bound = vars.names.len();
        let head_atom = self.read_atom(&head.line, &head.line.text, model, None, Some(&mut vars))?;
        if vars.names.len() > bound {
            return Err(Error::at(
                &head.line,
                format!(
                    "`?{}` appears in the conclusion but in none of the conditions",
                    vars.names[bound]
                ),
            ));
        }
        self.relations[head_atom.rel].derived = true;
        self.rules.push(Rule {
            name: name.to_string(),
            model,
            head: head_atom,
            body,
            vars: vars.names,
        });
        Ok(self.rules.len() - 1)
    }

    fn resolve_revision(
        &mut self,
        name: &str,
        pending: &HashMap<String, &Block>,
        visiting: &mut Vec<String>,
    ) -> Result<usize, Error> {
        if let Some(&w) = self.world_index.get(name) {
            return Ok(w);
        }
        let block = pending[name];
        if visiting.iter().any(|v| v == name) {
            return Err(Error::at(&block.line, "revisions can't be based on themselves"));
        }
        visiting.push(name.to_string());
        let Some(from) = block
            .children
            .first()
            .and_then(|c| c.line.text.strip_prefix("from:"))
            .map(str::trim)
        else {
            return Err(Error::at(&block.line, "a revision starts with `from: <model or revision>`"));
        };
        let base = match self.world_index.get(from) {
            Some(&w) => w,
            None if pending.contains_key(from) => self.resolve_revision(from, pending, visiting)?,
            None => {
                return Err(Error::at(&block.children[0].line, format!("no model or revision named `{from}`")));
            }
        };
        visiting.pop();

        let mut world = World {
            name: name.to_string(),
            model: self.worlds[base].model,
            base: Some(base),
            facts: self.worlds[base].facts.clone(),
            entities: self.worlds[base].entities.clone(),
            rules: self.worlds[base].rules.clone(),
            says: None,
            line: block.line.no,
        };
        let model = world.model;
        let sections = &block.children[1..];
        // Names first, so facts and rules anywhere in the revision can use them.
        for section in sections.iter().filter(|s| s.line.text == "admit:") {
            for item in self.items(&section.children)? {
                if let Item::Member { line, name, ty } = item {
                    let id = self.declare_entity(&line, model, &name, ty, false)?;
                    if !world.entities.insert(id) {
                        return Err(Error::at(&line, format!("`{name}` already exists in `{from}`")));
                    }
                }
            }
        }
        for section in sections {
            let text = section.line.text.as_str();
            if let Some(claim) = text.strip_prefix("says:") {
                if world.says.is_some() || !section.children.is_empty() {
                    return Err(Error::at(&section.line, "one `says:` line per revision"));
                }
                world.says = Some(claim.trim().trim_matches('"').to_string());
                continue;
            }
            if let Some(rule) = text.strip_prefix("add rule ").and_then(|r| r.strip_suffix(':')) {
                let id = self.read_rule(rule.trim(), &section.line, &section.children)?;
                if self.rules[id].model != model {
                    return Err(Error::at(&section.line, "the rule belongs to a different model"));
                }
                world.rules.insert(id);
                continue;
            }
            if let Some(rule) = text.strip_prefix("remove rule ") {
                let rule = rule.trim();
                let found = world.rules.iter().copied().find(|&r| self.rules[r].name == rule);
                match found {
                    Some(id) if section.children.is_empty() => {
                        world.rules.remove(&id);
                    }
                    _ => return Err(Error::at(&section.line, format!("no rule named `{rule}` in `{from}`"))),
                }
                continue;
            }
            let admit = match text {
                "admit:" => true,
                "withdraw:" => false,
                _ => {
                    return Err(Error::at(
                        &section.line,
                        "expected `says:`, `admit:`, `withdraw:`, `add rule <name>:` or `remove rule <name>`",
                    ));
                }
            };
            for item in self.items(&section.children)? {
                match item {
                    Item::Member { line, .. } if !admit => {
                        return Err(Error::at(&line, "names can be admitted but not withdrawn"));
                    }
                    Item::Member { .. } => {}
                    Item::Fact { line, text } => {
                        let fact = Self::ground(&self.read_atom(&line, &text, model, Some(&world.entities), None)?);
                        if admit && !world.facts.insert(fact.clone()) {
                            return Err(Error::at(&line, format!("this is already true in `{from}`")));
                        }
                        if !admit && !world.facts.remove(&fact) {
                            return Err(Error::at(&line, format!("this isn't asserted in `{from}`, so it can't be withdrawn")));
                        }
                    }
                }
            }
        }
        let id = self.worlds.len();
        self.world_index.insert(name.to_string(), id);
        self.worlds.push(world);
        Ok(id)
    }

    fn world_named(&self, line: &Line, name: &str) -> Result<usize, Error> {
        self.world(name)
            .ok_or_else(|| Error::at(line, format!("no model or revision named `{name}`")))
    }

    fn read_request(&self, block: &Block, next: Option<&Block>) -> Result<(RequestItem, bool), Error> {
        let line = &block.line;
        let text = line.text.as_str();
        let mut source = vec![text.to_string()];
        for child in &block.children {
            source.push(format!("  {}", child.line.text));
        }

        for verb in ["diff ", "review "] {
            let Some(rest) = text.strip_prefix(verb) else { continue };
            let (a, b) = rest.split_once(" -> ").unwrap();
            let base = self.world_named(line, a.trim())?;
            let successor = self.world_named(line, b.trim())?;
            if self.worlds[base].model != self.worlds[successor].model {
                return Err(Error::at(line, "only revisions of the same model can be compared"));
            }
            if !block.children.is_empty() {
                return Err(Error::at(line, format!("`{}` takes no indented lines", verb.trim())));
            }
            let request = if verb == "diff " {
                Request::Diff { base, successor }
            } else {
                Request::Review { base, successor }
            };
            return Ok((RequestItem { line: line.no, source, request }, false));
        }

        let Some(header) = text.strip_suffix(':') else {
            return Err(Error::at(line, "this question should end with `:`"));
        };
        let [clause] = block.children.as_slice() else {
            return Err(Error::at(line, "put exactly one sentence on the indented line below"));
        };
        if !clause.children.is_empty() {
            return Err(Error::at(&clause.line, "unexpected indentation"));
        }

        if let Some(rest) = header.strip_prefix("find all ?") {
            let (var, world) = rest
                .split_once(" in ")
                .ok_or_else(|| Error::at(line, "expected `find all ?x in <model>:`"))?;
            let world = self.world_named(line, world.trim())?;
            let w = &self.worlds[world];
            let mut vars = Vars::default();
            let pattern = self.read_atom(&clause.line, &clause.line.text, w.model, Some(&w.entities), Some(&mut vars))?;
            let var = vars
                .names
                .iter()
                .position(|n| n == var.trim())
                .ok_or_else(|| Error::at(line, format!("`?{}` doesn't appear in the sentence", var.trim())))?;
            let request = Request::Find {
                world,
                var,
                pattern,
                vars: vars.names,
            };
            return Ok((RequestItem { line: line.no, source, request }, false));
        }

        if let Some(rest) = header.strip_prefix("why not in ") {
            let world = self.world_named(line, rest.trim())?;
            let w = &self.worlds[world];
            let target = Self::ground(&self.read_atom(&clause.line, &clause.line.text, w.model, Some(&w.entities), None)?);
            let request = Request::WhyNot { world, target };
            return Ok((RequestItem { line: line.no, source, request }, false));
        }

        let (verb, pick, world_name) = if let Some(rest) = header.strip_prefix("why all in ") {
            ("why", Pick::All, rest)
        } else if let Some(rest) = header.strip_prefix("why in ") {
            ("why", Pick::One, rest)
        } else {
            let (verb, rest) = header.split_once(' ').unwrap();
            if let Some(rest) = rest.strip_prefix("all minimal in ") {
                (verb, Pick::All, rest)
            } else if let Some(rest) = rest.strip_prefix("one minimal in ") {
                (verb, Pick::One, rest)
            } else {
                return Err(Error::at(line, format!("expected `{verb} all minimal in …` or `{verb} one minimal in …`")));
            }
        };
        let world = self.world_named(line, world_name.trim())?;
        let w = &self.worlds[world];
        let target = Self::ground(&self.read_atom(&clause.line, &clause.line.text, w.model, Some(&w.entities), None)?);
        if verb == "why" {
            let request = Request::Why { world, target, pick };
            return Ok((RequestItem { line: line.no, source, request }, false));
        }

        let Some(using) = next else {
            return Err(Error::at(
                line,
                format!("`{verb}` needs a `using:` block naming the relations it may change"),
            ));
        };
        source.push("using:".into());
        let mut relations = Vec::new();
        for child in &using.children {
            let name = child.line.text.trim();
            let rel = self
                .relation(name)
                .ok_or_else(|| Error::at(&child.line, format!("no relation named `{name}`")))?;
            if self.relations[rel].derived {
                return Err(Error::at(
                    &child.line,
                    format!("`{name}` is concluded by rules; `using:` lists relations you state directly"),
                ));
            }
            if relations.contains(&rel) {
                return Err(Error::at(&child.line, "listed twice"));
            }
            relations.push(rel);
            source.push(format!("  {name}"));
        }
        if relations.is_empty() {
            return Err(Error::at(&using.line, "list at least one relation"));
        }
        let request = if verb == "prevent" {
            Request::Prevent { world, target, using: relations, pick }
        } else {
            Request::Achieve { world, target, using: relations, pick }
        };
        Ok((RequestItem { line: line.no, source, request }, true))
    }

    /// The closest declared name in `model`, if one is near enough to be a typo.
    fn suggest(&self, model: usize, name: &str) -> Option<String> {
        let target = name.to_lowercase();
        let limit = if target.chars().count() >= 5 { 2 } else { 1 };
        self.entity_index
            .keys()
            .filter(|(m, _)| *m == model)
            .map(|(_, candidate)| (edit_distance(&target, &candidate.to_lowercase()), candidate))
            .filter(|&(d, _)| d <= limit)
            .min()
            .map(|(_, candidate)| candidate.clone())
    }

    /// A rule as one line of text: `name: conclusion when a, b and c`.
    pub fn rule_text(&self, rule: usize) -> String {
        let r = &self.rules[rule];
        let mut body: Vec<String> = r.body.iter().map(|a| self.atom_sentence(a, &r.vars)).collect();
        let last = body.pop().unwrap_or_default();
        let conditions = if body.is_empty() { last } else { format!("{} and {last}", body.join(", ")) };
        format!("{}: {} when {conditions}", r.name, self.atom_sentence(&r.head, &r.vars))
    }

    /// Read one must-sentence.
    fn read_intent(&self, model: usize, line: &Line) -> Result<Intent, Error> {
        let text = line.text.trim();
        let make = |vars: Vars, kind| Intent {
            model,
            line: line.no,
            text: text.to_string(),
            vars: vars.names,
            kind,
        };
        if let Some(rest) = text.strip_prefix("never ") {
            let mut vars = Vars::default();
            let atom = self.read_atom(line, rest, model, None, Some(&mut vars))?;
            return Ok(make(vars, IntentKind::Never(atom)));
        }
        let mut whole_vars = Vars::default();
        let whole = self.read_atom(line, text, model, None, Some(&mut whole_vars));
        let mut options = Vec::new();
        let mut split_error = None;
        for keyword in [" requires ", " has no single point of failure among "] {
            for (at, _) in text.match_indices(keyword) {
                let (left, right) = (&text[..at], &text[at + keyword.len()..]);
                let mut vars = Vars::default();
                let parsed = self
                    .read_atom(line, left, model, None, Some(&mut vars))
                    .and_then(|a| Ok((a, self.read_atom(line, right, model, None, Some(&mut vars))?)));
                match parsed {
                    Ok((a, b)) => {
                        let kind = if keyword == " requires " {
                            IntentKind::Requires(a, b)
                        } else {
                            IntentKind::NoSinglePoint(a, b)
                        };
                        options.push(make(vars, kind));
                    }
                    Err(e) => {
                        split_error.get_or_insert(e);
                    }
                }
            }
        }
        match (whole, options.len()) {
            (Ok(atom), 0) => Ok(make(whole_vars, IntentKind::Holds(atom))),
            (Err(_), 1) => Ok(options.pop().unwrap()),
            (Err(e), 0) => Err(split_error.unwrap_or(e)),
            _ => Err(Error::at(line, "this must-sentence can be read more than one way")),
        }
    }

    /// Read one sentence against every declared shape. Exactly one reading
    /// must type-check; otherwise the error says why.
    fn read_atom(
        &self,
        line: &Line,
        text: &str,
        model: usize,
        visible: Option<&BTreeSet<usize>>,
        mut vars: Option<&mut Vars>,
    ) -> Result<Atom, Error> {
        let tokens: Vec<&str> = text.split_whitespace().collect();
        let mut found: Vec<(usize, Vec<Filler>)> = Vec::new();
        let mut misses: Vec<String> = Vec::new();

        for (rel, relation) in self.relations.iter().enumerate() {
            let mut ways = Vec::new();
            splits(&tokens, 0, &relation.template, 0, &mut Vec::new(), &mut ways);
            'way: for way in ways {
                let mut fillers = Vec::new();
                for (slot, &(a, b)) in way.iter().enumerate() {
                    let role = &relation.roles[slot];
                    let piece = tokens[a..b].join(" ");
                    if let Some(var) = piece.strip_prefix('?') {
                        if b - a != 1 || !is_identifier(var) {
                            misses.push(format!("`{piece}` isn't a valid variable"));
                            continue 'way;
                        }
                        let Some(table) = vars.as_deref() else {
                            misses.push(format!("`{piece}`: this line states a fact, so it can't use variables"));
                            continue 'way;
                        };
                        let earlier = table.type_of(var).or_else(|| {
                            fillers.iter().find_map(|f| match f {
                                Filler::Var(name, ty) if name == var => Some(*ty),
                                _ => None,
                            })
                        });
                        if let Some(ty) = earlier.filter(|&ty| ty != role.ty) {
                            misses.push(format!(
                                "`{piece}` is a {} elsewhere, but `{}` here needs a {}",
                                self.types[ty], role.name, self.types[role.ty]
                            ));
                            continue 'way;
                        }
                        fillers.push(Filler::Var(var.to_string(), role.ty));
                    } else {
                        let Some(&id) = self.entity_index.get(&(model, piece.clone())) else {
                            misses.push(match self.suggest(model, &piece) {
                                Some(near) => format!(
                                    "`{piece}` isn't declared in `{}`. Did you mean `{near}`?",
                                    self.models[model]
                                ),
                                None => format!(
                                    "`{piece}` isn't declared in `{}` (add `{piece} ∈ {}`)",
                                    self.models[model], self.types[role.ty]
                                ),
                            });
                            continue 'way;
                        };
                        if self.entities[id].ty != role.ty {
                            misses.push(format!(
                                "`{piece}` is a {}, but `{}` in \"{}\" needs a {}",
                                self.types[self.entities[id].ty],
                                role.name,
                                self.shape(rel),
                                self.types[role.ty]
                            ));
                            continue 'way;
                        }
                        if visible.is_some_and(|v| !v.contains(&id)) {
                            misses.push(format!("`{piece}` doesn't exist in this revision"));
                            continue 'way;
                        }
                        fillers.push(Filler::Const(id));
                    }
                }
                found.push((rel, fillers));
            }
        }

        match found.len() {
            0 => {
                if let Some(miss) = misses.first() {
                    Err(Error::at(line, miss.clone()))
                } else {
                    // Rank shapes by the words they share with this line, and by
                    // whether the names on this line fit their slots.
                    let named_types: Vec<usize> = self
                        .entity_index
                        .iter()
                        .filter(|((m, name), _)| {
                            let words: Vec<&str> = name.split_whitespace().collect();
                            *m == model && tokens.windows(words.len()).any(|w| w == words.as_slice())
                        })
                        .map(|(_, &id)| self.entities[id].ty)
                        .collect();
                    let mut ranked: Vec<(usize, usize)> = (0..self.relations.len())
                        .map(|r| {
                            let relation = &self.relations[r];
                            let words = relation
                                .template
                                .iter()
                                .filter(|p| matches!(p, Piece::Word(w) if tokens.contains(&w.as_str())))
                                .count();
                            let slots = relation.roles.iter().filter(|role| named_types.contains(&role.ty)).count();
                            let fits_all = slots == relation.roles.len();
                            (2 * words + slots + usize::from(fits_all), r)
                        })
                        .filter(|&(shared, _)| shared > 0)
                        .collect();
                    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
                    let closest: Vec<String> = ranked.iter().take(3).map(|&(_, r)| format!("  {}", self.shape(r))).collect();
                    let message = if closest.is_empty() {
                        "this doesn't match any declared sentence shape".to_string()
                    } else {
                        format!("this doesn't match any declared sentence shape. Closest:\n{}", closest.join("\n"))
                    };
                    Err(Error::at(line, message))
                }
            }
            1 => {
                let (rel, fillers) = found.pop().unwrap();
                let terms = fillers
                    .into_iter()
                    .map(|filler| match filler {
                        Filler::Const(id) => Term::Const(id),
                        Filler::Var(name, ty) => Term::Var(vars.as_deref_mut().unwrap().index(&name, ty)),
                    })
                    .collect();
                Ok(Atom { rel, terms })
            }
            _ => {
                let readings: Vec<String> = found
                    .iter()
                    .map(|(rel, _)| format!("  as {}: {}", self.relations[*rel].name, self.shape(*rel)))
                    .collect();
                Err(Error::at(
                    line,
                    format!("this can be read more than one way:\n{}", readings.join("\n")),
                ))
            }
        }
    }
}

/// Levenshtein distance between two strings, by characters.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for j in 0..b.len() {
            let current = row[j + 1];
            row[j + 1] = if ca == b[j] { previous } else { 1 + previous.min(row[j]).min(row[j + 1]) };
            previous = current;
        }
    }
    row[b.len()]
}
