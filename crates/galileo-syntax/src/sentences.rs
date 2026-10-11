use crate::handlers::{
    read_handler, read_question as read_question_line, read_requirements, Handler, Question,
    QuestionKind, Requirements,
};
use crate::rules::{read_rule, Rule};
use crate::{blocks, Block, Error, Line, Token, TokenKind};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Static,
    State,
    Derived,
    Event,
    Effect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    Word(String),
    Slot(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub name: String,
    pub ty: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mode {
    pub exactly: bool,
    pub value: usize,
    pub keys: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Domain {
    pub slot: usize,
    pub closed: bool,
    pub values: Vec<Arg>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    pub kind: Kind,
    pub checkpoint: bool,
    pub name: String,
    pub template: Vec<Piece>,
    pub slots: Vec<Slot>,
    pub modes: Vec<Mode>,
    pub domains: Vec<Domain>,
    pub one_per_step: bool,
    pub settled_by: Option<String>,
    pub line: usize,
    pub module: String,
    pub private: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Arg {
    Var(String),
    Name(String),
    Int(i64),
    Text(String),
    Path(String),
    List(Vec<Arg>),
    Expr(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reading {
    pub shape: usize,
    pub args: Vec<Arg>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fact {
    pub shape: usize,
    pub args: Vec<Arg>,
    pub line: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Vocabulary {
    pub types: BTreeMap<String, Option<Vec<String>>>,
    pub fields: BTreeMap<(String, String), String>,
    pub shapes: Vec<Shape>,
    pub entities: BTreeMap<String, BTreeSet<String>>,
    pub module: String,
    pub type_modules: BTreeMap<String, String>,
    pub type_names: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Clone, Debug, Default)]
pub struct Sentences {
    pub vocabulary: Vocabulary,
    pub facts: Vec<Fact>,
    pub entity_facts: Vec<Fact>,
    pub rules: Vec<Rule>,
    pub handlers: Vec<Handler>,
    pub requirements: Vec<Requirements>,
    pub uses: Vec<String>,
    pub exports: Vocabulary,
    pub warnings: Vec<Error>,
}

#[derive(Clone, Debug)]
pub struct Unit {
    pub tokens: Vec<Token>,
    pub text: String,
}

const BUILTIN_TYPES: [&str; 5] = ["Int", "Text", "Bool", "Path", "OptionPath"];
const LATER: [&str; 8] = [
    "use",
    "on",
    "requirements",
    "nix",
    "define",
    "view",
    "foreign",
    "patch",
];

impl fmt::Display for Arg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Arg::Var(name) => write!(f, "?{name}"),
            Arg::Name(name) | Arg::Path(name) | Arg::Expr(name) => write!(f, "{name}"),
            Arg::Int(n) => write!(f, "{n}"),
            Arg::Text(text) => write!(f, "\"{text}\""),
            Arg::List(items) => {
                let items: Vec<String> = items.iter().map(Arg::to_string).collect();
                write!(f, "[{}]", items.join(", "))
            }
        }
    }
}

pub type Declarations = Vec<(String, usize, String, String)>;

pub fn read_sentences(source: &str) -> Result<Sentences, Error> {
    read_module("", source, &BTreeMap::new(), &declarations("", source))
}

/// Every `X is a T` line of a module, as (module, line, name, type), read before any module is.
pub fn declarations(module: &str, source: &str) -> Declarations {
    let mut out = Vec::new();
    for block in blocks(source).unwrap_or_default() {
        let Ok(subject) = units(&block.line, &block.line.tokens) else {
            continue;
        };
        let mut sentences = Vec::new();
        if block.children.is_empty() {
            sentences.push((block.line.no, subject.clone()));
        }
        for child in block.children.iter().filter(|c| c.children.is_empty()) {
            if let Ok(rest) = units(&child.line, &child.line.tokens) {
                let mut sentence = subject.clone();
                sentence.extend(rest);
                sentences.push((child.line.no, sentence));
            }
        }
        for (line, sentence) in sentences {
            if let [name, is, a, ty] = sentence.as_slice() {
                if let (Some(names), "is", "a") = (declared_names(name), is.text.as_str(), a.text.as_str()) {
                    for n in names {
                        out.push((module.to_string(), line, n.clone(), ty.text.clone()));
                    }
                }
            }
        }
    }
    out
}

/// The names a declaration's subject declares: one name, or each element of a list of names.
fn declared_names(unit: &Unit) -> Option<Vec<&String>> {
    fn name(token: &Token) -> Option<&String> {
        match &token.kind {
            TokenKind::Word(n) | TokenKind::QuotedName(n) => Some(n),
            _ => None,
        }
    }
    let is = |token: &Token, s: &str| matches!(&token.kind, TokenKind::Symbol(t) if t == s);
    match unit.tokens.as_slice() {
        [single] => Some(vec![name(single)?]),
        [open, inner @ .., close] if is(open, "[") && is(close, "]") => {
            if inner.is_empty() {
                return Some(Vec::new());
            }
            if inner.len() % 2 == 0 || inner.iter().skip(1).step_by(2).any(|t| !is(t, ",")) {
                return None;
            }
            inner.iter().step_by(2).map(name).collect()
        }
        _ => None,
    }
}

pub fn read_question(
    source: &str,
    kind: QuestionKind,
    modules: &BTreeMap<String, Sentences>,
) -> Result<(Vocabulary, Question), Error> {
    let lines = crate::lex(source)?;
    let [line] = lines.as_slice() else {
        return Err(Error::bare("a question is one line"));
    };
    if source.lines().count() != 1 {
        return Err(Error::at(line, "a question is one line"));
    }
    let mut vocabulary = Vocabulary::default();
    for module in modules.values() {
        vocabulary
            .import(&module.exports)
            .map_err(|message| Error::at(line, message))?;
    }
    let question = read_question_line(&vocabulary, line, kind)?;
    Ok((vocabulary, question))
}

/// Reads every module, each after the modules it `use`s. A failure names its module.
pub fn read_program(
    sources: &[(&str, &str)],
) -> Result<BTreeMap<String, Sentences>, (String, Error)> {
    let mut uses: BTreeMap<&str, Vec<(String, Line)>> = BTreeMap::new();
    let mut vocabularies = BTreeMap::new();
    for (name, source) in sources {
        let top = blocks(source).map_err(|e| (name.to_string(), e))?;
        let mut vocabulary = Vocabulary { module: name.to_string(), ..Vocabulary::default() };
        let mut private_types = BTreeSet::new();
        for mut block in top.clone() {
            let private = word(&block.line, 0) == Some("private");
            if private {
                block.line.tokens.remove(0);
            }
            let before: BTreeSet<String> = vocabulary.types.keys().cloned().collect();
            if word(&block.line, 0) == Some("type") {
                vocabulary.declare_type(&block.line).map_err(|e| (name.to_string(), e))?;
            } else if word(&block.line, 0) == Some("state") && word(&block.line, 1) == Some("entity") {
                if let Some(ty) = word(&block.line, 2) {
                    vocabulary.types.insert(ty.into(), None);
                }
            }
            for ty in vocabulary.types.keys().filter(|ty| !before.contains(*ty)) {
                vocabulary.type_modules.insert(ty.clone(), name.to_string());
                vocabulary.type_names.insert(ty.clone(), BTreeSet::from([ty.clone()]));
                if private {
                    private_types.insert(ty.clone());
                }
            }
        }
        vocabularies.insert(*name, (vocabulary.clone(), vocabulary.exports(&private_types)));
        let listed = top
            .iter()
            .filter(|b| word(&b.line, 0) == Some("use"))
            .map(|b| (module_path(&b.line), b.line.clone()))
            .collect();
        uses.insert(name, listed);
    }
    let mut declared = Vec::new();
    for (name, source) in sources {
        let mut vocabulary = vocabularies[name].0.clone();
        for (used, _) in &uses[name] {
            if let Some((_, exports)) = vocabularies.get(used.as_str()) {
                vocabulary.import(exports).map_err(|e| (name.to_string(), Error::bare(e)))?;
            }
        }
        let lines = crate::lex(source).map_err(|e| (name.to_string(), e))?;
        for (module, line, entity, ty) in declarations(name, source) {
            let Some(at) = lines.iter().find(|at| at.no == line) else {
                continue;
            };
            let Ok(ty) = vocabulary.resolve_type(at, &ty) else {
                continue;
            };
            if !vocabulary.types.get(&ty).is_some_and(Option::is_none) {
                continue;
            }
            let canonical = vocabulary.canonical(&ty);
            declared.push((module, line, entity, canonical));
        }
    }
    let mut done = BTreeMap::new();
    for (name, _) in sources {
        visit(name, sources, &uses, &declared, &mut Vec::new(), &mut done)?;
    }
    Ok(done)
}

fn visit(
    name: &str,
    sources: &[(&str, &str)],
    uses: &BTreeMap<&str, Vec<(String, Line)>>,
    declared: &Declarations,
    path: &mut Vec<String>,
    done: &mut BTreeMap<String, Sentences>,
) -> Result<(), (String, Error)> {
    if done.contains_key(name) {
        return Ok(());
    }
    path.push(name.into());
    for (used, line) in &uses[name] {
        if let Some(at) = path.iter().position(|p| p == used) {
            let mut cycle = path[at..].to_vec();
            cycle.push(used.clone());
            return Err((
                name.into(),
                Error::at(
                    line,
                    format!("these modules use each other: {}", cycle.join(" → ")),
                ),
            ));
        }
        if uses.contains_key(used.as_str()) {
            visit(used, sources, uses, declared, path, done)?;
        }
    }
    path.pop();
    let source = sources.iter().find(|(n, _)| *n == name).unwrap().1;
    let read = read_module(name, source, done, declared).map_err(|e| (name.to_string(), e))?;
    done.insert(name.into(), read);
    Ok(())
}

fn module_path(line: &Line) -> String {
    line.tokens[1..].iter().map(token_text).collect()
}

/// Reads one module against its own declarations and the exports of the modules it `use`s.
pub fn read_module(
    name: &str,
    source: &str,
    modules: &BTreeMap<String, Sentences>,
    declared: &Declarations,
) -> Result<Sentences, Error> {
    let mut top = blocks(source)?;
    let mut out = Sentences::default();
    let mut private = BTreeSet::new();
    for block in &mut top {
        let is_private = word(&block.line, 0) == Some("private");
        if is_private {
            block.line.tokens.remove(0);
        }
        let first = word(&block.line, 0);
        if first == Some("static") {
            let message = if word(&block.line, 1) == Some("entity") {
                "`static entity Name` is now `type Name`; a mutable entity is `state entity Name`"
            } else {
                "the `static` shape kind is now `given`"
            };
            return Err(Error::at(&block.line, message));
        }
        if !is_private {
            continue;
        }
        if first == Some("type") || first == Some("define") || shape_kind(&block.line).is_some() {
            private.insert(block.line.no);
        } else if first == Some("state") && word(&block.line, 1) == Some("entity") {
            private.insert(block.line.no);
        } else {
            return Err(Error::at(
                &block.line,
                "only a type, a shape, an entity type or a `define` can be `private`",
            ));
        }
    }
    let v = &mut out.vocabulary;
    v.module = name.into();

    let mut private_types = BTreeSet::new();
    let mut entity_types = Vec::new();
    for block in &top {
        let before: BTreeSet<String> = v.types.keys().cloned().collect();
        if word(&block.line, 0) == Some("type") {
            v.declare_type(&block.line)?;
        } else if word(&block.line, 0) == Some("state")
            && word(&block.line, 1) == Some("entity")
        {
            let name = word(&block.line, 2)
                .ok_or_else(|| Error::at(&block.line, "expected `entity Name`"))?;
            v.types.insert(name.into(), None);
        }
        for ty in v.types.keys().filter(|t| !before.contains(*t)) {
            if v.types[ty].is_none() {
                let kind = if word(&block.line, 0) == Some("state") { Kind::State } else { Kind::Static };
                entity_types.push((ty.clone(), kind, block.line.no, private.contains(&block.line.no)));
            }
            v.type_modules.insert(ty.clone(), name.into());
            v.type_names.insert(ty.clone(), BTreeSet::from([ty.clone()]));
            if private.contains(&block.line.no) {
                private_types.insert(ty.clone());
            }
        }
    }
    for block in top.iter().filter(|b| word(&b.line, 0) == Some("use")) {
        let used = module_path(&block.line);
        if block.line.tokens.len() < 2 || !block.children.is_empty() {
            return Err(Error::at(
                &block.line,
                "`use` names one module, as in `use firn/tags`",
            ));
        }
        let Some(module) = modules.get(&used) else {
            return Err(Error::at(
                &block.line,
                format!("there is no module `{used}` to use"),
            ));
        };
        if !out.uses.contains(&used) {
            v.import(&module.exports)
                .map_err(|m| Error::at(&block.line, m))?;
            for (ty, origin) in &module.vocabulary.type_modules {
                if *origin == used && !module.exports.type_modules.contains_key(ty) {
                    continue;
                }
                let ty = ty.strip_prefix(&format!("{origin}/")).unwrap_or(ty);
                v.type_modules
                    .entry(format!("{origin}/{ty}"))
                    .or_insert_with(|| origin.clone());
            }
            out.uses.push(used);
        }
    }
    for block in &top {
        if matches!(word(&block.line, 0), Some("state" | "derived"))
            && matches!(word(&block.line, 1), Some("for" | "entity"))
        {
            v.declare_fields(block)?;
        }
        if let Some(kind) = shape_kind(&block.line) {
            v.declare_shape(kind, block, private.contains(&block.line.no))?;
            if !private.contains(&block.line.no) {
                for slot in &v.shapes.last().unwrap().slots {
                    if let Some(ty) = slot.ty.split(|c: char| c.is_whitespace() || c == '(' || c == ')')
                        .find(|ty| private_types.contains(*ty))
                    {
                        return Err(Error::at(&block.line, format!("a public shape may not mention private type `{ty}`")));
                    }
                }
            }
        } else if !private.contains(&block.line.no) && !private_types.is_empty() {
            if word(&block.line, 0) == Some("state") && word(&block.line, 1) == Some("entity") {
                for child in &block.children {
                    let tokens = &child.line.tokens;
                    if let [field, colon, ty @ ..] = tokens.as_slice() {
                        if symbol(colon, ":") {
                            for token in ty {
                                if let TokenKind::Word(ty) = &token.kind {
                                    if private_types.contains(ty) {
                                        return Err(Error::at(&child.line, format!("public field `{}` may not mention private type `{ty}`", token_text(field))));
                                    }
                                }
                            }
                        }
                    }
                }
            } else if word(&block.line, 0) == Some("define") {
                let signature = crate::check::signature(&block.line)?;
                let mut types: Vec<_> = signature.params.iter().map(|(_, ty)| ty).collect();
                types.push(&signature.result);
                while let Some(ty) = types.pop() {
                    match ty {
                        crate::types::Type::Maybe(inner) | crate::types::Type::Nix(inner) => types.push(inner),
                        crate::types::Type::Named { name, args, .. } => {
                            if private_types.contains(name) {
                                return Err(Error::at(&block.line, format!("public definition `{}` may not mention private type `{name}`", signature.name)));
                            }
                            types.extend(args);
                        }
                    }
                }
            }
        }
    }
    for (ty, kind, line, private) in entity_types {
        v.shapes.push(Shape {
            kind,
            checkpoint: false,
            name: format!("is-a-{ty}"),
            template: vec![Piece::Slot(0), Piece::Word("is".into()), Piece::Word("a".into()), Piece::Word(ty.clone())],
            slots: vec![Slot { name: "entity".into(), ty }],
            modes: Vec::new(),
            domains: Vec::new(),
            one_per_step: false,
            settled_by: None,
            line,
            module: name.into(),
            private,
        });
    }

    let mut sentences = Vec::new();
    for block in &top {
        let first = word(&block.line, 0).unwrap_or("");
        if first == "initially" && block.line.tokens.len() == 1 {
            for child in &block.children {
                focus(v, child, Kind::State, &mut sentences)?;
            }
        } else if fact_block(block) {
            focus(v, block, Kind::Static, &mut sentences)?;
        }
    }

    let mut facts_of = Vec::new();
    for (line, units, kind) in &sentences {
        if let Some((names, ty)) = v.entity_declaration(units, line)? {
            for name in names {
                let canonical = v.canonical(&ty);
                let written = v.written(&ty);
                let first = declared.iter().find(|(_, _, n, t)| *n == name && *t == canonical);
                if let Some((module, at, _, _)) = first.filter(|(m, at, _, _)| *m != v.module || *at != line.no) {
                    let place = if *module == v.module { String::new() } else { format!(" in `{module}`") };
                    out.warnings.push(Error::at(line, format!("`{name}` is already declared a {written}{place} on line {at}")));
                }
                if let Some(shape) = v.entity_shape(&ty) {
                    out.entity_facts.push(Fact {
                        shape,
                        args: vec![Arg::Name(name.clone())],
                        line: line.no,
                    });
                }
                let mut types = BTreeSet::from([ty.clone()]);
                for (ty, values) in &v.types {
                    if values.is_none() && declared.iter().any(|(_, _, entity, declared_ty)| {
                        *entity == name && *declared_ty == v.canonical(ty)
                    }) {
                        types.insert(ty.clone());
                    }
                }
                v.entities.entry(name).or_default().extend(types);
            }
        } else {
            facts_of.push((line, units, *kind));
        }
    }

    for (line, units, kind) in facts_of {
        let tokens = units.iter().flat_map(|unit| unit.tokens.clone()).collect::<Vec<_>>();
        let reading = match v.field_reading(line, &tokens, &mut BTreeMap::new())? {
            Some(reading) => reading,
            None => v.read(line, units, None)?,
        };
        let shape = &v.shapes[reading.shape];
        if shape.kind == Kind::Derived {
            return Err(Error::at(
                line,
                format!("`{}` is derived, so only rules give its facts", shape.name),
            ));
        }
        if shape.kind != kind {
            let message = match kind {
                Kind::Static => {
                    "only given facts are stated here; state facts go under `initially`"
                }
                _ => "only state facts go under `initially`",
            };
            return Err(Error::at(
                line,
                format!("{message} (`{}` is {:?})", shape.name, shape.kind),
            ));
        }
        for args in expand(&reading.args, &shape.slots) {
            out.facts.push(Fact {
                shape: reading.shape,
                args,
                line: line.no,
            });
        }
    }
    out.warnings.extend(check_facts(&out.vocabulary, &out.facts, &sentences)?);
    for block in top.iter().filter(|b| word(&b.line, 0) == Some("rule")) {
        let rule = read_rule(&out.vocabulary, block, &out.rules)?;
        out.rules.push(rule);
    }
    for block in &top {
        match word(&block.line, 0) {
            Some("on") => out.handlers.push(read_handler(&out.vocabulary, block)?),
            Some("requirements") => out
                .requirements
                .push(read_requirements(&out.vocabulary, block)?),
            _ => {}
        }
    }
    let v = &mut out.vocabulary;
    for (ty, values) in &v.types {
        if values.is_some() {
            continue;
        }
        let canonical = v.canonical(&ty);
        for (_, _, entity, declared_ty) in declared {
            if *declared_ty == canonical {
                v.entities.entry(entity.clone()).or_default().insert(ty.clone());
            }
        }
    }
    out.exports = out.vocabulary.exports(&private_types);
    Ok(out)
}

fn fact_block(block: &Block) -> bool {
    let first = word(&block.line, 0).unwrap_or("");
    first != "type"
        && first != "rule"
        && first != "private"
        && !(first == "initially" && block.line.tokens.len() == 1)
        && shape_kind(&block.line).is_none()
        && !LATER.contains(&first)
        && !(first == "state" && word(&block.line, 1) == Some("entity"))
        && !(matches!(first, "state" | "derived") && word(&block.line, 1) == Some("for"))
}

/// The line numbers of each fact table's rows, for `galileo fmt` to align.
pub(crate) fn table_rows(top: &[Block]) -> Vec<Vec<usize>> {
    let initially = top
        .iter()
        .filter(|b| word(&b.line, 0) == Some("initially") && b.line.tokens.len() == 1)
        .flat_map(|b| &b.children);
    top.iter()
        .filter(|b| fact_block(b))
        .chain(initially)
        .flat_map(|b| std::iter::once(b).chain(&b.children))
        .filter(|b| !b.children.is_empty())
        .filter(|b| units(&b.line, &b.line.tokens).is_ok_and(|u| u.iter().any(is_variable)))
        .map(|b| b.children.iter().map(|c| c.line.no).collect())
        .collect()
}

fn is_variable(unit: &Unit) -> bool {
    matches!(unit.tokens.as_slice(), [token] if matches!(token.kind, TokenKind::Variable(_)))
}

fn focus(v: &Vocabulary, block: &Block, kind: Kind, out: &mut Vec<(Line, Vec<Unit>, Kind)>) -> Result<(), Error> {
    let subject = units(&block.line, &block.line.tokens)?;
    if subject.iter().any(is_variable) {
        return table(v, block, subject, None, kind, out);
    }
    if block.children.is_empty() {
        out.push((block.line.clone(), subject, kind));
        return Ok(());
    }
    for child in &block.children {
        let line = &child.line;
        let mut sentence = subject.clone();
        if let [subject_unit] = subject.as_slice() {
            if let [field, equal, ..] = line.tokens.as_slice() {
                if matches!(field.kind, TokenKind::Word(_)) && symbol(equal, "=") {
                    if let Some(deeper) = child.children.first() {
                        return Err(Error::at(&deeper.line, "a field fact holds no indented lines"));
                    }
                    let mut dot = field.clone();
                    dot.kind = TokenKind::Symbol(".".into());
                    let mut tokens = subject_unit.tokens.clone();
                    tokens.push(dot);
                    tokens.extend(line.tokens.iter().cloned());
                    sentence = vec![Unit { tokens, text: format!("{}.{}", subject_unit.text, line.text.trim()) }];
                    out.push((line.clone(), sentence, kind));
                    continue;
                }
            }
        }
        let predicate = units(line, &line.tokens)?;
        let has_variable = predicate.iter().any(is_variable);
        sentence.extend(predicate);
        if has_variable {
            let fixed = arg(&block.line, &subject[0])?;
            table(v, child, sentence, Some(fixed), kind, out)?;
            continue;
        }
        if let Some(deeper) = child.children.first() {
            return Err(Error::at(&deeper.line, "focus blocks nest only one level"));
        }
        out.push((line.clone(), sentence, kind));
    }
    Ok(())
}

fn table(
    v: &Vocabulary,
    block: &Block,
    header: Vec<Unit>,
    mut subject: Option<Arg>,
    kind: Kind,
    out: &mut Vec<(Line, Vec<Unit>, Kind)>,
) -> Result<(), Error> {
    let line = &block.line;
    if block.children.is_empty() {
        let vars: Vec<String> = header.iter().filter(|u| is_variable(u)).map(|u| format!("`{}`", u.text)).collect();
        return Err(Error::at(
            line,
            format!("{}: a fact holds no variables, and a fact table header needs its rows indented under it", vars.join(", ")),
        ));
    }
    let reading = v.read(line, &header, Some(&mut BTreeMap::new()))?;
    let mut names = Vec::new();
    for arg in &reading.args {
        match arg {
            Arg::Var(name) if name == "_" => {
                return Err(Error::at(line, "a fact table header names each slot; `?_` fills none"))
            }
            Arg::Var(name) if names.contains(name) => {
                return Err(Error::at(line, format!("a fact table header names each slot once; `?{name}` is repeated")))
            }
            Arg::Var(name) => names.push(name.clone()),
            other if subject.as_ref() == Some(other) => subject = None,
            other => {
                return Err(Error::at(line, format!("a fact table header fills every slot with a variable; `{other}` is not one")))
            }
        }
    }
    let columns: Vec<String> = names.iter().map(|n| format!("?{n}")).collect();
    for child in &block.children {
        let row = &child.line;
        if let Some(deeper) = child.children.first() {
            return Err(Error::at(&deeper.line, "a fact table row holds no indented lines"));
        }
        if row.tokens.iter().any(|t| matches!(t.kind, TokenKind::TextBlock(_))) {
            return Err(Error::at(row, "a fact table row holds no text block; state that fact on its own line"));
        }
        let cells = units(row, &row.tokens)?;
        if cells.len() != names.len() {
            return Err(Error::at(
                row,
                format!("this row gives {} tokens, but the table's header takes {}: {}", cells.len(), names.len(), columns.join(" ")),
            ));
        }
        let mut cells = cells.into_iter();
        let sentence = header
            .iter()
            .map(|unit| if is_variable(unit) { cells.next().unwrap() } else { unit.clone() })
            .collect();
        out.push((row.clone(), sentence, kind));
    }
    Ok(())
}

fn expand(args: &[Arg], slots: &[Slot]) -> Vec<Vec<Arg>> {
    let mut out = vec![Vec::new()];
    for (arg, slot) in args.iter().zip(slots) {
        let choices = match arg {
            Arg::List(items) if !holds_list(&slot.ty) => items.clone(),
            other => vec![other.clone()],
        };
        out = out
            .into_iter()
            .flat_map(|prefix| {
                choices.iter().map(move |choice| {
                    let mut next = prefix.clone();
                    next.push(choice.clone());
                    next
                })
            })
            .collect();
    }
    out
}

fn check_facts(
    v: &Vocabulary,
    facts: &[Fact],
    sentences: &[(Line, Vec<Unit>, Kind)],
) -> Result<Vec<Error>, Error> {
    let line_of = |no: usize| &sentences.iter().find(|(l, _, _)| l.no == no).unwrap().0;
    let mut warnings = Vec::new();
    let mut seen: BTreeMap<(usize, &[Arg]), usize> = BTreeMap::new();
    let mut keyed: BTreeMap<(usize, usize, Vec<&Arg>), &Fact> = BTreeMap::new();
    for fact in facts {
        if let Some(&first) = seen.get(&(fact.shape, fact.args.as_slice())) {
            warnings.push(Error::at(
                line_of(fact.line),
                format!("this fact is already stated on line {first}"),
            ));
            continue;
        }
        seen.insert((fact.shape, &fact.args), fact.line);
        let shape = &v.shapes[fact.shape];
        for (index, mode) in shape.modes.iter().enumerate() {
            let key: Vec<&Arg> = mode.keys.iter().map(|&k| &fact.args[k]).collect();
            if let Some(other) = keyed.get(&(fact.shape, index, key.clone())) {
                return Err(Error::at(
                    line_of(fact.line),
                    format!(
                        "these facts break `{}`:\n  line {}: {}\n  line {}: {}",
                        v.mode_text(fact.shape, mode),
                        other.line,
                        v.sentence(other.shape, &other.args),
                        fact.line,
                        v.sentence(fact.shape, &fact.args),
                    ),
                ));
            }
            keyed.insert((fact.shape, index, key), fact);
        }
    }
    Ok(warnings)
}

fn word_is(token: Option<&Token>, text: &str) -> bool {
    token.is_some_and(|t| matches!(&t.kind, TokenKind::Word(w) if w == text))
}

pub(crate) fn list_item(ty: &str) -> Option<&str> {
    let inner = ty.strip_prefix("List ")?;
    Some(inner.strip_prefix('(').and_then(|i| i.strip_suffix(')')).unwrap_or(inner))
}

fn base_type(ty: &str) -> &str {
    match (ty.strip_prefix("maybe "), list_item(ty)) {
        (Some(inner), _) | (None, Some(inner)) => base_type(inner),
        _ => ty,
    }
}

pub(crate) fn holds_list(ty: &str) -> bool {
    ty.starts_with("nix ") || ty.starts_with("List ")
}

fn word(line: &Line, index: usize) -> Option<&str> {
    match &line.tokens.get(index)?.kind {
        TokenKind::Word(w) => Some(w),
        _ => None,
    }
}

fn symbol(token: &Token, text: &str) -> bool {
    matches!(&token.kind, TokenKind::Symbol(s) if s == text)
}

fn shape_kind(line: &Line) -> Option<Kind> {
    let kind = match word(line, 0)? {
        "given" => Kind::Static,
        "state" => Kind::State,
        "derived" => Kind::Derived,
        "event" => Kind::Event,
        "effect" => Kind::Effect,
        _ => return None,
    };
    (!matches!(word(line, 1), Some("entity" | "for"))).then_some(kind)
}

fn token_text(token: &Token) -> String {
    match &token.kind {
        TokenKind::Word(w) | TokenKind::Integer(w) | TokenKind::Path(w) | TokenKind::Symbol(w) => {
            w.clone()
        }
        TokenKind::Variable(v) => format!("?{v}"),
        TokenKind::QuotedName(n) => format!("`{n}`"),
        TokenKind::Computed(e) => format!("`{{{e}}}`"),
        TokenKind::Text(t) => format!("\"{t}\""),
        TokenKind::TextBlock(_) => "'''".into(),
    }
}

/// Splits a line into slot tokens: balanced brackets and adjacent tokens are one unit.
pub fn units(line: &Line, tokens: &[Token]) -> Result<Vec<Unit>, Error> {
    let mut out: Vec<Unit> = Vec::new();
    let mut depth: Vec<&str> = Vec::new();
    let mut previous_end = None;
    for token in tokens {
        let joined = !depth.is_empty() || previous_end == Some(token.span.start);
        let opens_after_space = !joined;
        match &token.kind {
            TokenKind::Symbol(s) if matches!(s.as_str(), "(" | "[" | "{") => {
                depth.push(match s.as_str() {
                    "(" => ")",
                    "[" => "]",
                    _ => "}",
                })
            }
            TokenKind::Symbol(s) if matches!(s.as_str(), ")" | "]" | "}") => {
                if depth.pop() != Some(s.as_str()) {
                    return Err(Error::at(line, format!("unbalanced `{s}`")));
                }
            }
            _ => {}
        }
        if opens_after_space || out.is_empty() {
            out.push(Unit {
                tokens: Vec::new(),
                text: String::new(),
            });
        } else if previous_end != Some(token.span.start) {
            out.last_mut().unwrap().text.push(' ');
        }
        let unit = out.last_mut().unwrap();
        unit.text.push_str(&token_text(token));
        unit.tokens.push(token.clone());
        previous_end = Some(token.span.end);
    }
    if let Some(close) = depth.last() {
        return Err(Error::at(line, format!("missing `{close}`")));
    }
    Ok(out)
}

fn arg(line: &Line, unit: &Unit) -> Result<Arg, Error> {
    let tokens = &unit.tokens;
    let expression = crate::expr::parse_expr(tokens, line)?;
    if matches!(
        expression.kind,
        crate::expr::ExprKind::Binary(..)
            | crate::expr::ExprKind::Not(..)
            | crate::expr::ExprKind::Neg(..)
            | crate::expr::ExprKind::If(..)
            | crate::expr::ExprKind::With(..)
            | crate::expr::ExprKind::Lambda(..)
            | crate::expr::ExprKind::Query(..)
            | crate::expr::ExprKind::NixModule(..)
    ) && !(symbol(&tokens[0], "(") && closes_at_end(tokens))
    {
        return Err(Error::at(
            line,
            "a slot takes one postfix expression; parenthesize an expression with an operator",
        ));
    }
    if let [token] = tokens.as_slice() {
        return Ok(match &token.kind {
            TokenKind::Variable(v) => Arg::Var(v.clone()),
            TokenKind::Word(w) if w == "_" => {
                return Err(Error::at(
                    line,
                    "a bare `_` isn't a filler; write `?_` for an anonymous variable",
                ))
            }
            TokenKind::Word(w) => Arg::Name(w.clone()),
            TokenKind::QuotedName(w) => Arg::Name(crate::lexer::quoted_name(w).map_err(|m| Error::at(line, m))?.into()),
            TokenKind::Integer(n) => match n.parse() {
                Ok(n) => Arg::Int(n),
                Err(_) => return Err(Error::at(line, format!("`{n}` is too large"))),
            },
            TokenKind::Text(t) | TokenKind::TextBlock(t) => Arg::Text(t.clone()),
            TokenKind::Path(p) => Arg::Path(p.clone()),
            TokenKind::Symbol(_) | TokenKind::Computed(_) => Arg::Expr(unit.text.clone()),
        });
    }
    let last = tokens.len() - 1;
    if symbol(&tokens[0], "[") && symbol(&tokens[last], "]") && closes_at_end(tokens) {
        let mut items = Vec::new();
        let mut start = 1;
        let mut depth = 0;
        for (i, token) in tokens.iter().enumerate().take(last).skip(1) {
            match &token.kind {
                TokenKind::Symbol(s) if matches!(s.as_str(), "(" | "[" | "{") => depth += 1,
                TokenKind::Symbol(s) if matches!(s.as_str(), ")" | "]" | "}") => depth -= 1,
                TokenKind::Symbol(s) if s == "," && depth == 0 => {
                    items.push(item(line, &tokens[start..i])?);
                    start = i + 1;
                }
                _ => {}
            }
        }
        if start < last {
            items.push(item(line, &tokens[start..last])?);
        }
        return Ok(Arg::List(items));
    }
    if tokens.iter().enumerate().all(|(i, t)| {
        if i % 2 == 0 {
            matches!(t.kind, TokenKind::Word(_))
        } else {
            symbol(t, ".")
        }
    }) && tokens.len() % 2 == 1
    {
        return Ok(Arg::Name(unit.text.clone()));
    }
    Ok(Arg::Expr(unit.text.clone()))
}

fn closes_at_end(tokens: &[Token]) -> bool {
    let mut depth = 0;
    for (i, token) in tokens.iter().enumerate() {
        match &token.kind {
            TokenKind::Symbol(s) if matches!(s.as_str(), "(" | "[" | "{") => depth += 1,
            TokenKind::Symbol(s) if matches!(s.as_str(), ")" | "]" | "}") => {
                depth -= 1;
                if depth == 0 && i != tokens.len() - 1 {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

fn ground(arg: &Arg) -> bool {
    match arg {
        Arg::Var(_) => false,
        Arg::List(items) => items.iter().all(ground),
        _ => true,
    }
}

fn item(line: &Line, tokens: &[Token]) -> Result<Arg, Error> {
    match units(line, tokens)?.as_slice() {
        [unit] => arg(line, unit),
        _ => Err(Error::at(line, "each list element is one token")),
    }
}

impl Vocabulary {
    fn declare_type(&mut self, line: &Line) -> Result<(), Error> {
        let tokens = &line.tokens[1..];
        let expected = || {
            Error::at(
                line,
                "expected `type Name, Name` or `type Name = \"a\" | \"b\"`",
            )
        };
        for token in tokens {
            if let TokenKind::Word(name) = &token.kind {
                if !name.chars().next().is_some_and(char::is_uppercase) {
                    let mut chars = name.chars();
                    let capitalized = chars.next().unwrap().to_uppercase().collect::<String>() + chars.as_str();
                    return Err(Error::at(line, format!("a type's name begins with a capital; write `type {capitalized}`")));
                }
            }
        }
        if tokens.get(1).is_some_and(|t| symbol(t, "=")) {
            let Some(TokenKind::Word(name)) = tokens.first().map(|t| &t.kind) else {
                return Err(expected());
            };
            let mut values = Vec::new();
            for (i, token) in tokens[2..].iter().enumerate() {
                match (&token.kind, i % 2) {
                    (TokenKind::Text(t), 0) => values.push(t.clone()),
                    (TokenKind::Symbol(s), 1) if s == "|" => {}
                    _ => return Err(expected()),
                }
            }
            if values.is_empty() || tokens.len() % 2 == 0 {
                return Err(expected());
            }
            self.types.insert(name.clone(), Some(values));
            return Ok(());
        }
        for (i, token) in tokens.iter().enumerate() {
            match (&token.kind, i % 2) {
                (TokenKind::Word(name), 0) => {
                    self.types.insert(name.clone(), None);
                }
                (TokenKind::Symbol(s), 1) if s == "," => {}
                _ => return Err(expected()),
            }
        }
        if tokens.is_empty() || tokens.len() % 2 == 0 {
            return Err(expected());
        }
        Ok(())
    }

    fn declare_fields(&mut self, block: &Block) -> Result<(), Error> {
        let line = &block.line;
        let Some(written) = word(line, 2).filter(|_| line.tokens.len() == 3) else {
            return Err(Error::at(line, "a field block is `state for T` or `derived for T`"));
        };
        let ty = self.resolve_type(line, written)?;
        if !self.types.get(&ty).is_some_and(Option::is_none) {
            return Err(Error::at(line, format!("fields need an entity type, but `{written}` isn't one")));
        }
        let identity = self.canonical(&ty);
        for child in &block.children {
            let tokens = &child.line.tokens;
            let [name, colon, field_ty @ ..] = tokens.as_slice() else {
                return Err(Error::at(&child.line, "a field is `name: Type`"));
            };
            let TokenKind::Word(name) = &name.kind else {
                return Err(Error::at(&child.line, "a field is `name: Type`"));
            };
            if !symbol(colon, ":") || field_ty.is_empty() || !child.children.is_empty() {
                return Err(Error::at(&child.line, "a field is `name: Type`"));
            }
            crate::types::parse_type(field_ty, &child.line)?;
            let key = (identity.clone(), name.clone());
            if self.fields.contains_key(&key) {
                return Err(Error::at(&child.line, format!("field `{written}.{name}` is declared twice; a field's name is unique across its type's blocks")));
            }
            let field_type = field_ty.iter().map(token_text).collect::<Vec<_>>().join(" ");
            self.fields.insert(key, field_type.clone());
            if word(line, 1) == Some("for") {
                let optional = field_type.strip_prefix("maybe ");
                self.shapes.push(Shape {
                    kind: if word(line, 0) == Some("derived") { Kind::Derived } else { Kind::State },
                    checkpoint: false,
                    name: format!("{written}.{name}"),
                    template: Vec::new(),
                    slots: vec![Slot { name: written.into(), ty: ty.clone() }, Slot { name: name.clone(), ty: optional.unwrap_or(&field_type).into() }],
                    modes: vec![Mode { exactly: optional.is_none(), value: 1, keys: vec![0] }],
                    domains: Vec::new(),
                    one_per_step: false,
                    settled_by: None,
                    line: child.line.no,
                    module: self.module.clone(),
                    private: false,
                });
            }
        }
        Ok(())
    }

    fn declare_shape(&mut self, kind: Kind, block: &Block, private: bool) -> Result<(), Error> {
        let line = &block.line;
        let mut rest = &line.tokens[1..];
        if word(line, 1) == Some("checkpoint") {
            return Err(Error::at(line, "the `checkpoint` shape flag is now `milestone`"));
        }
        let checkpoint = rest
            .first()
            .is_some_and(|t| matches!(&t.kind, TokenKind::Word(w) if w == "milestone"));
        if checkpoint {
            rest = &rest[1..];
        }
        let mut name = None;
        if let [first, colon, ..] = rest {
            if let (TokenKind::Word(w), true) = (&first.kind, symbol(colon, ":")) {
                name = Some(w.clone());
                rest = &rest[2..];
            }
        }
        let mut template = Vec::new();
        let mut slots: Vec<Slot> = Vec::new();
        let mut bare = BTreeSet::new();
        let mut i = 0;
        while i < rest.len() {
            match &rest[i].kind {
                TokenKind::Word(w) if w.chars().next().is_some_and(char::is_uppercase) => {
                    if BUILTIN_TYPES.contains(&w.as_str()) {
                        return Err(Error::at(line, format!("a prelude type needs a named slot; write `{{name: {w}}}`")));
                    }
                    let ty = self.resolve_type(line, w).map_err(|mut error| {
                        if !self.is_type(w) {
                            if let Some(nearest) = self.type_names.keys()
                                .min_by_key(|name| crate::check::distance(w, name))
                            {
                                error.message.push_str(&format!("; the nearest type is `{nearest}`"));
                            }
                            error.message.push_str(&format!("; for a literal word, write `{w}` in backticks"));
                        }
                        error
                    })?;
                    if slots.iter().any(|slot| slot.ty == ty) {
                        return Err(Error::at(line, format!("two slots have type `{w}`; name both explicitly, as `{{from: {w}}}` and `{{to: {w}}}`")));
                    }
                    let name = w.chars().enumerate().flat_map(|(at, c)| {
                        let prefix = if at > 0 && c.is_uppercase() { "-" } else { "" };
                        prefix.chars().chain(c.to_lowercase())
                    }).collect();
                    bare.insert(ty.clone());
                    template.push(Piece::Slot(slots.len()));
                    slots.push(Slot { name, ty });
                }
                TokenKind::Word(w) => template.push(Piece::Word(w.clone())),
                TokenKind::QuotedName(w) => template.push(Piece::Word(crate::lexer::quoted_name(w).map_err(|m| Error::at(line, m))?.into())),
                TokenKind::Variable(_) | TokenKind::Symbol(_) if matches!(&rest[i].kind, TokenKind::Variable(_)) || symbol(&rest[i], "{") => {
                    let slot = if let TokenKind::Variable(name) = &rest[i].kind {
                        if !rest.get(i + 1).is_some_and(|token| symbol(token, ":")) {
                            return Err(Error::at(line, format!("a template blank needs a type; write `?{name}: Type`")));
                        }
                        let start = i + 2;
                        let mut end = start;
                        while rest.get(end).is_some_and(|token| matches!(&token.kind, TokenKind::Word(w) if w == "maybe" || w == "nix")) {
                            end += 1;
                        }
                        let Some(Token { kind: TokenKind::Word(ty), .. }) = rest.get(end) else {
                            return Err(Error::at(line, format!("a template blank needs a type; write `?{name}: Type`")));
                        };
                        end += 1;
                        if matches!(ty.as_str(), "List" | "Attrs") {
                            let arguments = units(line, &rest[end..])?;
                            let Some(argument) = arguments.first() else {
                                return Err(Error::at(line, format!("`{ty}` takes one type argument")));
                            };
                            end += argument.tokens.len();
                        }
                        let ty = self.slot_type(line, &rest[start..end])?;
                        if self.type_names.contains_key(&ty) {
                            let bare_name: String = ty.chars().enumerate().flat_map(|(at, c)| {
                                let prefix = if at > 0 && c.is_uppercase() { "-" } else { "" };
                                prefix.chars().chain(c.to_lowercase())
                            }).collect();
                            if *name == bare_name {
                                return Err(Error::at(line, format!("the type names this slot; write `{ty}` instead of `?{name}: {ty}`")));
                            }
                        }
                        i = end - 1;
                        Slot { name: name.clone(), ty }
                    } else {
                        let close = rest[i..].iter().position(|t| symbol(t, "}")).map(|p| p + i);
                        match (close, rest.get(i + 1).map(|t| &t.kind), rest.get(i + 2)) {
                            (Some(close), Some(TokenKind::Word(slot)), Some(colon))
                                if symbol(colon, ":") && close > i + 3 =>
                            {
                                let ty = self.slot_type(line, &rest[i + 3..close])?;
                                i = close;
                                Slot {
                                    name: slot.clone(),
                                    ty,
                                }
                            }
                            _ => return Err(Error::at(line, "a slot is written `{name: Type}`")),
                        }
                    };
                    if slots.iter().any(|s| s.name == slot.name) {
                        return Err(Error::at(
                            line,
                            format!("two slots are named `{}`", slot.name),
                        ));
                    }
                    if bare.contains(&slot.ty) {
                        let ty = self.written(&slot.ty);
                        return Err(Error::at(line, format!("two slots have type `{ty}`; name both explicitly, as `{{from: {ty}}}` and `{{to: {ty}}}`")));
                    }
                    template.push(Piece::Slot(slots.len()));
                    slots.push(slot);
                }
                _ => {
                    return Err(Error::at(
                        line,
                        format!(
                            "a template holds words and `{{slot: Type}}`, not `{}`",
                            token_text(&rest[i])
                        ),
                    ))
                }
            }
            i += 1;
        }
        let words: Vec<&str> = template
            .iter()
            .filter_map(|p| match p {
                Piece::Word(w) => Some(w.as_str()),
                Piece::Slot(_) => None,
            })
            .collect();
        if words.is_empty() {
            return Err(Error::at(line, "a template needs at least one word"));
        }
        let name = name.unwrap_or_else(|| words.join("-"));
        if let Some(other) = self
            .shapes
            .iter()
            .find(|s| s.name == name && s.module == self.module)
        {
            return Err(Error::at(
                line,
                format!(
                    "a shape named `{name}` is declared on line {}; name one with `name:`",
                    other.line
                ),
            ));
        }
        let mut shape = Shape {
            kind,
            checkpoint,
            name,
            template,
            slots,
            modes: Vec::new(),
            domains: Vec::new(),
            one_per_step: false,
            settled_by: None,
            line: line.no,
            module: self.module.clone(),
            private,
        };
        for child in &block.children {
            if let Some(deeper) = child.children.first() {
                return Err(Error::at(
                    &deeper.line,
                    "a shape line takes no indented block; write each shape line one level under the shape",
                ));
            }
            self.shape_line(&mut shape, &child.line)?;
        }
        self.shapes
            .retain(|other| other.module == self.module || other.name != shape.name);
        self.shapes.push(shape);
        Ok(())
    }

    fn shape_line(&self, shape: &mut Shape, line: &Line) -> Result<(), Error> {
        let words: Vec<String> = line.tokens.iter().map(token_text).collect();
        let words: Vec<&str> = words.iter().map(String::as_str).collect();
        let slot = |name: &str| {
            shape
                .slots
                .iter()
                .position(|s| s.name == name)
                .ok_or_else(|| Error::at(line, format!("this shape has no slot `{name}`")))
        };
        match words.as_slice() {
            ["at", "most", "one", "per", "step"] if shape.kind == Kind::Effect => shape.one_per_step = true,
            ["settled", "by", event] if shape.kind == Kind::Effect => shape.settled_by = Some(event.to_string()),
            ["exactly" | "at", ..] if words.contains(&"per") => {
                let (exactly, rest) = match words.as_slice() {
                    ["exactly", "one", rest @ ..] => (true, rest),
                    ["at", "most", "one", rest @ ..] => (false, rest),
                    _ => return Err(Error::at(line, "a mode is `exactly one X per K` or `at most one X per K`")),
                };
                let [value, "per", keys @ ..] = rest else {
                    return Err(Error::at(line, "a mode is `exactly one X per K` or `at most one X per K`"));
                };
                let mut key_slots = Vec::new();
                for (i, key) in keys.iter().enumerate() {
                    match (i % 2, *key) {
                        (0, key) => key_slots.push(slot(key)?),
                        (1, ",") => {}
                        _ => return Err(Error::at(line, "separate key slots with commas")),
                    }
                }
                if key_slots.is_empty() || keys.len() % 2 == 0 {
                    return Err(Error::at(line, "a mode names its key slots after `per`"));
                }
                if exactly && key_slots.len() > 1 {
                    return Err(Error::at(line, "a key of several slots takes `at most one`"));
                }
                shape.modes.push(Mode { exactly, value: slot(value)?, keys: key_slots });
            }
            [name, "only", "in", ..] | [name, "sampled", "from", ..] => {
                let index = slot(name)?;
                let closed = words[1] == "only";
                let list = units(line, &line.tokens[3..])?;
                let values = match list.as_slice() {
                    [unit] => match arg(line, unit)? {
                        Arg::List(values) => values,
                        _ => return Err(Error::at(line, "a domain is a bracketed list")),
                    },
                    _ => return Err(Error::at(line, "a domain is a bracketed list")),
                };
                for value in &values {
                    let mut vars = BTreeMap::new();
                    if let Err(miss) = self.fits(value, &shape.slots[index].ty, &mut vars) {
                        return Err(Error::at(line, miss));
                    }
                }
                shape.domains.push(Domain { slot: index, closed, values });
            }
            _ => {
                return Err(Error::at(
                    line,
                    "expected a mode (`exactly one X per K`), a domain (`X only in [..]`) or, on an effect, `at most one per step` or `settled by EVENT`",
                ))
            }
        }
        Ok(())
    }

    fn import(&mut self, exports: &Vocabulary) -> Result<(), String> {
        self.fields.extend(exports.fields.clone());
        let mut rename = BTreeMap::new();
        for (ty, values) in &exports.types {
            let origin = &exports.type_modules[ty];
            let original = ty.strip_prefix(&format!("{origin}/")).unwrap_or(ty);
            let local = format!("{origin}/{original}");
            rename.insert(ty.clone(), local.clone());
            self.types.insert(local.clone(), values.clone());
            self.type_modules.insert(local, origin.clone());
        }
        let local = |ty: &String| {
            let base = base_type(ty);
            match (rename.get(base), ty.rfind(base)) {
                (Some(renamed), Some(at)) => format!("{}{renamed}{}", &ty[..at], &ty[at + base.len()..]),
                _ => ty.clone(),
            }
        };
        for (name, candidates) in &exports.type_names {
            if self.type_modules.get(name) == Some(&self.module) {
                continue;
            }
            self.type_names
                .entry(name.clone())
                .or_default()
                .extend(candidates.iter().map(local));
        }
        for shape in &exports.shapes {
            if self
                .shapes
                .iter()
                .any(|s| s.module == shape.module && s.name == shape.name)
            {
                continue;
            }
            let mut shape = shape.clone();
            for slot in &mut shape.slots {
                slot.ty = local(&slot.ty);
            }
            self.shapes.push(shape);
        }
        Ok(())
    }

    fn exports(&self, private_types: &BTreeSet<String>) -> Vocabulary {
        let mut out = Vocabulary {
            module: self.module.clone(),
            ..Vocabulary::default()
        };
        let mut wanted: BTreeSet<String> = self
            .types
            .iter()
            .filter(|(ty, _)| {
                self.type_modules.get(*ty) == Some(&self.module) && !private_types.contains(*ty)
            })
            .map(|(ty, _)| ty.clone())
            .collect();
        for shape in self
            .shapes
            .iter()
            .filter(|s| s.module == self.module && !s.private)
        {
            for slot in &shape.slots {
                let base = base_type(&slot.ty);
                if self.types.contains_key(base) {
                    wanted.insert(base.to_string());
                }
            }
            out.shapes.push(shape.clone());
        }
        for ty in wanted {
            if self.type_modules[&ty] == self.module && !private_types.contains(&ty) {
                out.type_names
                    .insert(ty.clone(), BTreeSet::from([ty.clone()]));
            }
            out.types.insert(ty.clone(), self.types[&ty].clone());
            out.type_modules
                .insert(ty.clone(), self.type_modules[&ty].clone());
        }
        out.fields = self.fields.iter().filter(|((ty, _), _)| {
            !private_types.iter().any(|private| self.canonical(private) == *ty)
        }).map(|(key, ty)| (key.clone(), ty.clone())).collect();
        out
    }

    pub(crate) fn canonical(&self, ty: &str) -> String {
        match self.type_modules.get(ty) {
            Some(origin) if !origin.is_empty() && !ty.starts_with(&format!("{origin}/")) => format!("{origin}/{ty}"),
            _ => ty.to_string(),
        }
    }

    pub(crate) fn written<'a>(&self, ty: &'a str) -> &'a str {
        self.type_modules.get(ty).and_then(|origin| ty.strip_prefix(&format!("{origin}/"))).unwrap_or(ty)
    }

    pub(crate) fn resolve_type(&self, line: &Line, ty: &str) -> Result<String, Error> {
        if ty.starts_with("nix ") || self.type_modules.get(ty) == Some(&self.module) {
            return Ok(ty.into());
        }
        if let Some((outer, inner)) = ty.split_once(' ') {
            if matches!(outer, "maybe" | "List" | "Attrs") {
                let grouped = inner.strip_prefix('(').and_then(|inner| inner.strip_suffix(')'));
                let resolved = self.resolve_type(line, grouped.unwrap_or(inner).trim())?;
                return Ok(if grouped.is_some() {
                    format!("{outer} ({resolved})")
                } else {
                    format!("{outer} {resolved}")
                });
            }
        }
        let candidates = self.type_names.get(ty);
        let builtin = BUILTIN_TYPES.contains(&ty);
        let count = candidates.map_or(0, BTreeSet::len) + usize::from(builtin);
        if count == 1 {
            return Ok(candidates
                .and_then(|names| names.first())
                .cloned()
                .unwrap_or_else(|| ty.into()));
        }
        let mut origins: BTreeSet<String> = candidates
            .into_iter()
            .flatten()
            .map(|name| self.type_modules[name].clone())
            .collect();
        if builtin {
            origins.insert("prelude".into());
        }
        if count > 1 {
            return Err(Error::at(
                line,
                format!(
                    "`{ty}` is a type in both {}",
                    origins.into_iter().collect::<Vec<_>>().join(" and ")
                ),
            ));
        }
        origins.extend(self.type_modules.iter().filter_map(|(name, origin)| {
            (name.strip_prefix(&format!("{origin}/")) == Some(ty)).then(|| origin.clone())
        }));
        Err(Error::at(
            line,
            if origins.is_empty() {
                format!("`{ty}` isn't a declared type")
            } else {
                format!(
                    "`{ty}` is declared in {}; use its module directly to name it",
                    origins.into_iter().collect::<Vec<_>>().join(" and ")
                )
            },
        ))
    }

    fn slot_type(&self, line: &Line, tokens: &[Token]) -> Result<String, Error> {
        if word_is(tokens.first(), "nix") {
            return Ok(tokens.iter().map(token_text).collect::<Vec<_>>().join(" "));
        }
        let ty = crate::types::parse_type(tokens, line)?;
        self.slot_type_text(line, &ty)
    }

    fn slot_type_text(&self, line: &Line, ty: &crate::types::Type) -> Result<String, Error> {
        use crate::types::Type;
        match ty {
            Type::Maybe(inner) => Ok(format!("maybe {}", self.slot_type_text(line, inner)?)),
            Type::Named { name, args, .. } if name == "List" && args.len() == 1 => {
                let inner = self.slot_type_text(line, &args[0])?;
                Ok(if inner.contains(' ') { format!("List ({inner})") } else { format!("List {inner}") })
            }
            Type::Named { name, args, .. } if args.is_empty() => self.resolve_type(line, name),
            _ => Err(Error::at(line, "a slot's type is a type name, `List T`, `maybe T` or `nix T`")),
        }
    }

    pub(crate) fn is_type(&self, ty: &str) -> bool {
        ty.starts_with("nix ")
            || BUILTIN_TYPES.contains(&ty)
            || self.type_names.contains_key(ty)
            || self
                .type_modules
                .iter()
                .any(|(name, origin)| name.strip_prefix(&format!("{origin}/")) == Some(ty))
    }

    fn entity_declaration(
        &self,
        units: &[Unit],
        line: &Line,
    ) -> Result<Option<(Vec<String>, String)>, Error> {
        let declaration = match units {
            [name, is, a, ty] if is.text == "is" && a.text == "a" && self.is_type(&ty.text) =>
            {
                let ty = self.resolve_type(line, &ty.text)?;
                if !self.types.get(&ty).is_some_and(Option::is_none) {
                    return Ok(None);
                }
                match declared_names(name) {
                    Some(names) if names.is_empty() => {
                        return Err(Error::at(line, "`[]` declares no entity; a list in a declaration needs at least one element"));
                    }
                    Some(names) => Some((
                        names
                            .into_iter()
                            .map(|n| crate::lexer::quoted_name(n).map(str::to_string).map_err(|m| Error::at(line, m)))
                            .collect::<Result<Vec<_>, _>>()?,
                        ty,
                    )),
                    None => None,
                }
            }
            _ => None,
        };
        if let Some((_, ty)) = &declaration {
            if self.entity_shape(ty).is_some_and(|shape| self.shapes[shape].kind == Kind::State) {
                return Err(Error::at(line, format!("`{ty}` is a `state entity`; create it with `?x is a new {ty}` in a handler's `then` block")));
            }
        }
        Ok(declaration)
    }

    /// The shape `{entity: T} is a T` of entity type `T`.
    pub fn entity_shape(&self, ty: &str) -> Option<usize> {
        self.types.get(ty)?.is_none().then_some(())?;
        self.shapes.iter().position(|s| Self::is_entity_shape(s) && s.slots[0].ty == ty)
    }

    fn is_entity_shape(shape: &Shape) -> bool {
        matches!(shape.template.as_slice(), [Piece::Slot(0), Piece::Word(is), Piece::Word(a), Piece::Word(t)]
            if is == "is" && a == "a" && shape.name == format!("is-a-{t}"))
    }

    pub fn shape_text(&self, shape: usize) -> String {
        let shape = &self.shapes[shape];
        let words: Vec<String> = shape
            .template
            .iter()
            .map(|p| match p {
                Piece::Word(w) => w.clone(),
                Piece::Slot(s) => format!("{{{}}}", shape.slots[*s].name),
            })
            .collect();
        words.join(" ")
    }

    pub fn sentence(&self, shape: usize, args: &[Arg]) -> String {
        let words: Vec<String> = self.shapes[shape]
            .template
            .iter()
            .map(|p| match p {
                Piece::Word(w) => w.clone(),
                Piece::Slot(s) => args[*s].to_string(),
            })
            .collect();
        words.join(" ")
    }

    fn mode_text(&self, shape: usize, mode: &Mode) -> String {
        let slots = &self.shapes[shape].slots;
        let keys: Vec<&str> = mode.keys.iter().map(|&k| slots[k].name.as_str()).collect();
        let quantity = if mode.exactly {
            "exactly one"
        } else {
            "at most one"
        };
        format!(
            "{quantity} {} per {}",
            slots[mode.value].name,
            keys.join(", ")
        )
    }

    pub(crate) fn fits(&self, arg: &Arg, ty: &str, vars: &mut BTreeMap<String, String>) -> Result<(), String> {
        if ty.starts_with("nix ") {
            return Ok(());
        }
        match arg {
            Arg::Var(name) => match vars.get(name) {
                Some(earlier) if name != "_" && earlier.strip_prefix("maybe ") == Some(ty) => Err(format!(
                    "`?{name}` is a {earlier}, which may be `null`, but here needs a {ty}; state it in a `maybe {ty}` field instead, as in `rule r: ?x.f = …` with `f: maybe {ty}`"
                )),
                Some(earlier) if earlier != ty && name != "_" => Err(format!(
                    "`?{name}` is a {earlier} elsewhere, but here needs a {ty}"
                )),
                _ => {
                    vars.insert(name.clone(), ty.into());
                    Ok(())
                }
            },
            _ if ty.starts_with("maybe ") => self.fits(arg, &ty["maybe ".len()..], vars),
            Arg::Name(n) if ty != "OptionPath" && n.split_once('.').is_some_and(|(root, _)| !crate::check::FACT_ROOTS.contains(&root)) => {
                Err(format!("`{n}` is a dotted name, which fills only an `OptionPath` slot, not a {}", self.written(ty)))
            }
            Arg::Int(_) if ty == "Int" => Ok(()),
            Arg::Text(_) if ty == "Text" => Ok(()),
            Arg::Text(t)
                if self
                    .types
                    .get(ty)
                    .is_some_and(|u| u.as_ref().is_some_and(|u| u.contains(t))) =>
            {
                Ok(())
            }
            Arg::Path(_) if ty == "Path" => Ok(()),
            Arg::Name(n) if ty == "Bool" && (n == "true" || n == "false") => Ok(()),
            Arg::Name(n) => match self.entities.get(n) {
                Some(declared) if declared.contains(ty) => Ok(()),
                Some(declared) => {
                    let declared: Vec<&str> = declared.iter().map(|ty| self.written(ty)).collect();
                    let ty = self.written(ty);
                    Err(format!("`{n}` is a {}, but here needs a {ty}", declared.join(" and a ")))
                }
                None if ty == "OptionPath" || self.types.get(ty).is_some_and(Option::is_none) => {
                    Ok(())
                }
                None => Err(format!("`{n}` isn't a {ty}")),
            },
            Arg::List(items) => match list_item(ty) {
                Some(item) => items.iter().try_for_each(|arg| self.fits(arg, item, vars)),
                None => Err(format!("`{arg}` is a list, which fits only a `List` or `nix` slot, not a {}", self.written(ty))),
            },
            Arg::Expr(_) => Ok(()),
            other => Err(format!("`{other}` isn't a {ty}")),
        }
    }

    pub(crate) fn field_reading(
        &self,
        line: &Line,
        tokens: &[Token],
        vars: &mut BTreeMap<String, String>,
    ) -> Result<Option<Reading>, Error> {
        let [subject, dot, field, equal, value @ ..] = tokens else { return Ok(None); };
        if !symbol(dot, ".") || !symbol(equal, "=") { return Ok(None); }
        let TokenKind::Word(field) = &field.kind else { return Ok(None); };
        let subject = arg(line, &Unit { tokens: vec![subject.clone()], text: token_text(subject) })?;
        let mut candidates = Vec::new();
        for ((entity, name), field_type) in &self.fields {
            if name != field { continue; }
            let ty = self.types.keys().find(|ty| self.canonical(ty) == *entity).unwrap_or(entity);
            let mut table = vars.clone();
            if self.fits(&subject, ty, &mut table).is_err() { continue; }
            candidates.push((entity, field_type, table));
        }
        let [(entity, field_type, table)] = candidates.as_slice() else {
            return Err(Error::at(line, format!("`{subject}.{field}` needs one declared field of its entity type")));
        };
        let Some(shape) = self.shapes.iter().position(|shape| shape.template.is_empty()
            && shape.slots[1].name == *field && self.canonical(&shape.slots[0].ty) == **entity) else {
            let written = self.written(entity);
            return Err(Error::at(line, format!("`{written}.{field}` is a creation field, fixed at `new`; declare it under `state for {written}` to state another value")));
        };
        crate::expr::parse_expr(value, line)?;
        let text = units(line, value)?.iter().map(|unit| unit.text.as_str()).collect::<Vec<_>>().join(" ");
        if !field_type.starts_with("maybe ") && self.maybe_value(line, value, vars) {
            return Err(Error::at(line, format!("`{text}` may be `null`, but `{field}` is not a `maybe` field; declare it `{field}: maybe {field_type}`")));
        }
        if text == "null" && !field_type.starts_with("maybe ") {
            return Err(Error::at(line, format!("`{subject}.{field} = null` withdraws only a `maybe` field")));
        }
        *vars = table.clone();
        Ok(Some(Reading { shape, args: vec![subject, Arg::Expr(text)] }))
    }

    fn maybe_value(&self, line: &Line, value: &[Token], vars: &BTreeMap<String, String>) -> bool {
        match crate::expr::parse_expr(value, line).map(|e| e.kind) {
            Ok(crate::expr::ExprKind::Query(crate::expr::Query::Aggregate { op: crate::Aggregate::Min | crate::Aggregate::Max, .. })) => true,
            Ok(crate::expr::ExprKind::Var(name)) => vars.get(&name).is_some_and(|ty| ty.starts_with("maybe ")),
            _ => false,
        }
    }

    /// Reads one sentence against every shape. Exactly one reading must type-check.
    pub fn read(
        &self,
        line: &Line,
        units: &[Unit],
        vars: Option<&mut BTreeMap<String, String>>,
    ) -> Result<Reading, Error> {
        let mut found: Vec<(Reading, BTreeMap<String, String>)> = Vec::new();
        let mut misses: Vec<String> = Vec::new();

        'shape: for (index, shape) in self.shapes.iter().enumerate() {
            if shape.template.is_empty() { continue; }
            let Some(way) = split(units, &shape.template) else {
                continue;
            };
            let mut table = vars.as_deref().cloned().unwrap_or_default();
            let mut args = Vec::new();
            for (slot, &at) in way.iter().enumerate() {
                let unit = &units[at];
                let filler = arg(line, unit)?;
                if !ground(&filler) && vars.is_none() {
                    misses.push(format!(
                        "`{}`: this line states a fact, so it can't use variables",
                        unit.text
                    ));
                    continue 'shape;
                }
                let ty = &shape.slots[slot].ty;
                let fits = match &filler {
                    Arg::List(items) if vars.is_none() && !holds_list(ty) => {
                        if items.is_empty() {
                            Err("`[]` states no fact; a list in a fact needs at least one element".to_string())
                        } else {
                            items.iter().try_for_each(|item| self.fits(item, ty, &mut table))
                        }
                    }
                    _ => self.fits(&filler, ty, &mut table),
                };
                if let Err(miss) = fits {
                    misses.push(format!(
                        "{miss} for `{}` in \"{}\"",
                        shape.slots[slot].name,
                        self.shape_text(index)
                    ));
                    continue 'shape;
                }
                args.push(filler);
            }
            found.push((Reading { shape: index, args }, table));
        }

        match found.len() {
            0 => Err(Error::at(
                line,
                match misses.into_iter().next() {
                    Some(miss) => miss,
                    None => self.closest(units),
                },
            )),
            1 => {
                let (reading, table) = found.pop().unwrap();
                if let Some(vars) = vars {
                    *vars = table;
                }
                Ok(reading)
            }
            _ => {
                let readings: Vec<String> = found
                    .iter()
                    .map(|(r, _)| {
                        let shape = &self.shapes[r.shape];
                        let from = if shape.module == self.module {
                            String::new()
                        } else {
                            format!(" from {}", shape.module)
                        };
                        format!("  as {}{from}: {}", shape.name, self.shape_text(r.shape))
                    })
                    .collect();
                Err(Error::at(
                    line,
                    format!(
                        "this can be read more than one way:\n{}",
                        readings.join("\n")
                    ),
                ))
            }
        }
    }

    fn closest(&self, units: &[Unit]) -> String {
        let texts: Vec<&str> = units.iter().map(|u| u.text.as_str()).collect();
        let named_types: Vec<&String> =
            texts.iter().filter_map(|t| self.entities.get(*t)).flatten().collect();
        let mut ranked: Vec<(usize, usize)> = self
            .shapes
            .iter()
            .enumerate()
            .map(|(index, shape)| {
                let words = shape
                    .template
                    .iter()
                    .filter(|p| matches!(p, Piece::Word(w) if texts.contains(&w.as_str())))
                    .count();
                let slots = shape
                    .slots
                    .iter()
                    .filter(|s| named_types.contains(&&s.ty))
                    .count();
                let fits_all = slots == shape.slots.len();
                (2 * words + slots + usize::from(fits_all), index)
            })
            .filter(|&(score, _)| score > 0)
            .collect();
        if ranked.is_empty() {
            ranked = self.shapes.iter().enumerate().map(|(index, shape)| {
                let near = shape.template.iter().filter(|piece| {
                    matches!(piece, Piece::Word(w) if texts.iter().any(|t| {
                        !t.starts_with('?') && crate::check::distance(t, w) <= 1
                    }))
                }).count();
                (near, index)
            }).filter(|&(score, _)| score > 0).collect();
        }
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let closest: Vec<String> = ranked
            .iter()
            .take(3)
            .map(|&(_, s)| format!("  {}", self.shape_text(s)))
            .collect();
        if closest.is_empty() {
            "this doesn't match any declared sentence shape".into()
        } else {
            format!(
                "this doesn't match any declared sentence shape. Closest:\n{}",
                closest.join("\n")
            )
        }
    }
}

/// The one way to lay `units` over `template`, one unit per slot, as the unit index per slot.
fn split(units: &[Unit], template: &[Piece]) -> Option<Vec<usize>> {
    if units.len() != template.len() {
        return None;
    }
    let mut way = Vec::new();
    for (at, piece) in template.iter().enumerate() {
        match piece {
            Piece::Word(w) => {
                let unit = &units[at];
                if unit.tokens.len() != 1
                    || !matches!(&unit.tokens[0].kind, TokenKind::Word(t) if t == w)
                {
                    return None;
                }
            }
            Piece::Slot(_) => way.push(at),
        }
    }
    Some(way)
}
