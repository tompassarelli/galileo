use crate::check::{signature, Signature};
use crate::expr::{parse_expr, BinOp, Expr, ExprKind, Param, Query, TextPart};
use crate::record::Segment;
use crate::types::Type;
use crate::{
    blocks, lex, read_program, read_sentences, Action, Arg, Block, Claim, Error, Kind, Line, Literal, Quantifier,
    Reading, Sentences, Token, TokenKind,
};
use galileo_core as ir;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct Lowered {
    pub program: ir::Program,
    pub readings: Vec<SentenceReading>,
    pub diagnostics: Vec<Error>,
    pub warnings: Vec<Error>,
    /// Each `nix module` or `nix file` `for` query, by file and the source offset of its header.
    pub for_queries: Vec<(String, usize, ir::Query)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Role {
    Shape,
    Fact,
    Head { rule: String },
    Condition { rule: String, negated: bool },
    Query,
    Event,
    Handler { event: String, negated: bool },
    Claim { negated: bool },
}

#[derive(Clone)]
enum Context {
    Rule(String),
    Query,
    Handler(String),
    Claim,
}

/// What hover shows for one sentence: the shape it was read as and what fills each slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SentenceReading {
    pub line: usize,
    pub role: Role,
    pub relation: Option<ir::RelationId>,
    pub shape: String,
    pub template: String,
    pub slots: Vec<SlotReading>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotReading {
    pub name: String,
    pub ty: String,
    pub filler: Option<String>,
}

pub fn lower(source: &str, file: &str) -> Result<Lowered, Error> {
    let starts = [(1, file.to_string(), 0)];
    let mut lowered = lower_joined(source, &starts)?;
    for (name, _, _) in &mut lowered.for_queries {
        *name = file.to_string();
    }
    Ok(lowered)
}

fn place(starts: &[(usize, String, usize)], line: usize) -> (&str, usize, usize) {
    match starts.iter().rev().find(|(start, _, _)| *start <= line) {
        Some((start, file, offset)) => (file, line - start + 1, *offset),
        None => ("", line, 0),
    }
}

/// Lowers each module against its own vocabulary and links them into one Core IR, where
/// everything a module declares is named `module/name`.
pub fn lower_program(modules: &[(&str, &str)]) -> Result<Lowered, Error> {
    let read = read_program(modules).map_err(|(module, e)| named(&module, e))?;
    let mut order = Vec::new();
    for (name, _) in modules {
        dependencies_first(name, &read, &mut order);
    }
    let mut link = Link::default();
    let mut readings = Vec::new();
    let mut diagnostics = Vec::new();
    let mut warnings = Vec::new();
    let mut for_queries = Vec::new();
    let mut lines = BTreeMap::new();
    for name in &order {
        let source = modules.iter().find(|(n, _)| n == name).map_or("", |(_, s)| s);
        let starts = [(1, name.clone(), 0)];
        let part = lower_module(&read[name], source, &starts, &mut link).map_err(|e| named(name, e))?;
        diagnostics.extend(part.diagnostics.into_iter().map(|e| named(name, e)));
        warnings.extend(part.warnings.into_iter().map(|e| named(name, e)));
        readings.extend(part.readings);
        for_queries.extend(part.for_queries.into_iter().map(|(_, line, q)| (name.clone(), line, q)));
        lines.insert(name.clone(), part.lines);
    }
    let program = link.builder.build();
    if let Err(errors) = ir::check(&program) {
        diagnostics.extend(errors.into_iter().map(|mut e| match e.source.take() {
            Some(span) => match lines.get(&span.file).and_then(|l| l.get(&span.line)) {
                Some(line) => {
                    if let Some(owner) = e.owner.strip_prefix(&format!("{}/", span.file)) {
                        e.owner = owner.to_string();
                    }
                    named(&span.file, Error::at(line, e.to_string()))
                }
                None => {
                    e.source = Some(span);
                    Error::bare(e.to_string())
                }
            },
            None => Error::bare(e.to_string()),
        }));
    }
    Ok(Lowered {
        program,
        readings,
        diagnostics,
        warnings,
        for_queries,
    })
}

fn dependencies_first(name: &str, read: &BTreeMap<String, Sentences>, order: &mut Vec<String>) {
    if order.iter().any(|o| o == name) {
        return;
    }
    for used in &read[name].uses {
        dependencies_first(used, read, order);
    }
    order.push(name.into());
}

fn named(module: &str, mut error: Error) -> Error {
    error.message = format!("{module}: {}", error.message);
    error
}

#[derive(Default)]
struct Link {
    builder: ir::ProgramBuilder,
    relations: BTreeMap<(String, String), ir::RelationId>,
    types: BTreeSet<String>,
    entities: BTreeSet<(String, String)>,
    defines: BTreeMap<String, Vec<String>>,
    entity_fields: BTreeMap<String, Vec<ir::Slot>>,
}

struct Part {
    readings: Vec<SentenceReading>,
    diagnostics: Vec<Error>,
    warnings: Vec<Error>,
    for_queries: Vec<(String, usize, ir::Query)>,
    lines: BTreeMap<usize, Line>,
}

fn lower_joined(source: &str, starts: &[(usize, String, usize)]) -> Result<Lowered, Error> {
    let parsed = read_sentences(source)?;
    let mut link = Link::default();
    let mut part = lower_module(&parsed, source, starts, &mut link)?;
    let program = link.builder.build();
    if let Err(errors) = ir::check(&program) {
        part.diagnostics.extend(errors.into_iter().map(|mut e| match e.source.take() {
            Some(span) => match part.lines.get(&span.line) {
                Some(line) => {
                    if let Some(owner) = e.owner.strip_prefix(&format!("{}/", span.file)) {
                        e.owner = owner.to_string();
                    }
                    Error::at(line, e.to_string())
                }
                None => {
                    e.source = Some(span);
                    Error::bare(e.to_string())
                }
            },
            None => Error::bare(e.to_string()),
        }));
    }
    Ok(Lowered {
        program,
        readings: part.readings,
        diagnostics: part.diagnostics,
        warnings: part.warnings,
        for_queries: part.for_queries,
    })
}

fn lower_module(
    parsed: &Sentences,
    source: &str,
    starts: &[(usize, String, usize)],
    link: &mut Link,
) -> Result<Part, Error> {
    let lines = lex(source)?;
    let top: Vec<Block> = blocks(source)?
        .into_iter()
        .map(|mut b| {
            if word(&b.line.tokens, 0) == "private" {
                b.line.tokens.remove(0);
            }
            b
        })
        .collect();
    let signatures = top
        .iter()
        .filter(|b| word(&b.line.tokens, 0) == "define")
        .map(|b| signature(&b.line))
        .collect::<Result<Vec<_>, _>>()?;
    let heads = top
        .iter()
        .filter(|b| word(&b.line.tokens, 0) == "rule")
        .map(|b| (b.line.no, b.line.no))
        .collect();
    let module = parsed.vocabulary.module.clone();
    let own: Vec<String> = top
        .iter()
        .filter(|b| word(&b.line.tokens, 0) == "define")
        .map(|b| word(&b.line.tokens, 1).to_string())
        .collect();
    let mut calls: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for name in &own {
        calls.insert(name.clone(), vec![qualify(&module, name)]);
    }
    for used in &parsed.uses {
        for name in link.defines.get(used).into_iter().flatten() {
            if !own.contains(name) {
                calls.entry(name.clone()).or_default().push(qualify(used, name));
            }
        }
    }
    link.defines.insert(module.clone(), own);
    let mut lowering = Lowering {
        parsed,
        signatures: &signatures,
        lines: lines.iter().map(|l| (l.no, l)).collect(),
        starts,
        readings: Vec::new(),
        diagnostics: Vec::new(),
        params: BTreeSet::new(),
        view_params: BTreeMap::new(),
        handler_args: BTreeMap::new(),
        anonymous: 0,
        hoist: None,
        hoisted: 0,
        query_locals: Vec::new(),
        known: BTreeMap::new(),
        module,
        ids: Vec::new(),
        calls,
    };
    lowering.program(&heads, &top, link);
    let mut for_queries = Vec::new();
    for block in top.iter().filter(|b| word(&b.line.tokens, 0) == "nix") {
        lowering.for_queries(&block.children, &[], &mut for_queries);
    }
    Ok(Part {
        readings: lowering.readings,
        diagnostics: lowering.diagnostics,
        warnings: parsed.warnings.clone(),
        for_queries,
        lines: lines.iter().map(|l| (l.no, l.clone())).collect(),
    })
}

fn qualify(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.into()
    } else {
        format!("{module}/{name}")
    }
}

struct Lowering<'a> {
    query_locals: Vec<(usize, Expr, Vec<String>)>,
    parsed: &'a Sentences,
    signatures: &'a [Signature],
    lines: BTreeMap<usize, &'a Line>,
    starts: &'a [(usize, String, usize)],
    readings: Vec<SentenceReading>,
    diagnostics: Vec<Error>,
    params: BTreeSet<String>,
    view_params: BTreeMap<String, String>,
    handler_args: BTreeMap<String, String>,
    anonymous: usize,
    known: BTreeMap<String, ir::Value>,
    module: String,
    ids: Vec<ir::RelationId>,
    calls: BTreeMap<String, Vec<String>>,
    hoist: Option<Vec<ir::Literal>>,
    hoisted: usize,
}

impl Lowering<'_> {
    fn program(&mut self, heads: &BTreeMap<usize, usize>, top: &[Block], link: &mut Link) {
        let v = &self.parsed.vocabulary;
        let p = &mut link.builder;
        for (name, union) in &v.types {
            let identity = if v.module.is_empty() { name.clone() } else { format!("{}/{name}", v.module) };
            if union.is_some() || v.type_modules.get(name) != Some(&v.module) || !link.types.insert(identity.clone()) {
                continue;
            }
            let record = top.iter().find(|b| {
                word(&b.line.tokens, 0) == "state"
                    && word(&b.line.tokens, 1) == "entity"
                    && word(&b.line.tokens, 2) == name
            });
            match record {
                Some(block) => {
                    let fields = self.fields(block);
                    link.entity_fields.insert(identity.clone(), fields.clone());
                    p.state_entity_type(identity.clone(), fields);
                }
                _ => {
                    p.entity_type(identity);
                }
            }
        }
        for fact in &self.parsed.entity_facts {
            let (ty, Some(crate::Arg::Name(name))) = (&v.shapes[fact.shape].slots[0].ty, fact.args.first()) else {
                continue;
            };
            let ty = v.canonical(&ty);
            if link.entities.insert((ty.clone(), name.clone())) {
                p.entity(ir::Entity::named(ty.clone(), name.clone()));
            }
        }
        let mut effects = Vec::new();
        for (index, shape) in v.shapes.iter().enumerate() {
            if shape.module != v.module {
                self.ids.push(link.relations[&(shape.module.clone(), shape.name.clone())]);
                continue;
            }
            let slots = shape
                .slots
                .iter()
                .map(|s| ir::Slot::new(s.name.clone(), self.ty(&s.ty, shape.line)))
                .collect::<Vec<_>>();
            let mut relation = ir::Relation::new(qualify(&v.module, &shape.name), kind(shape.kind), slots)
                .template(v.shape_text(index))
                .at(self.span(shape.line));
            if shape.checkpoint {
                relation = relation.checkpoint();
            }
            for mode in &shape.modes {
                relation = relation.mode(if mode.exactly {
                    ir::Mode::ExactlyOne {
                        key: mode.keys[0],
                        values: vec![mode.value],
                    }
                } else {
                    ir::Mode::AtMostOne {
                        keys: mode.keys.clone(),
                        values: vec![mode.value],
                    }
                });
            }
            let id = p.relation(relation);
            link.relations.insert((v.module.clone(), shape.name.clone()), id);
            self.ids.push(id);
            self.read(shape.line, Role::Shape, index, None);
            match shape.kind {
                Kind::Event => {
                    let mut event = ir::Event::new(id);
                    for domain in &shape.domains {
                        let ty = &shape.slots[domain.slot].ty;
                        let values = domain
                            .values
                            .iter()
                            .map(|a| self.value(a, ty, shape.line, &ir::Program::default()))
                            .collect();
                        event = event.domain(
                            domain.slot,
                            if domain.closed {
                                ir::ArgumentDomain::Only(values)
                            } else {
                                ir::ArgumentDomain::Sampled(values)
                            },
                        );
                    }
                    p.event(event);
                }
                Kind::Effect => {
                    let mut effect = ir::Effect::new(id);
                    if shape.one_per_step {
                        effect = effect.at_most_one_per_step();
                    }
                    effects.push((effect, shape));
                }
                _ => {}
            }
        }
        for block in top.iter().filter(|b| {
            word(&b.line.tokens, 0) == "state" && word(&b.line.tokens, 1) == "entity"
        }) {
            let name = word(&block.line.tokens, 2);
            let ty = v.resolve_type(&block.line, name).unwrap();
            let identity = v.canonical(&ty);
            let fields = if word(&block.line.tokens, 1) == "entity" {
                link.entity_fields[&identity].clone()
            } else {
                self.fields(block)
            };
            for field in &fields {
                let (ty, mode) = match &field.ty {
                    ir::Type::Maybe(ty) => (
                        ty.as_ref().clone(),
                        ir::Mode::AtMostOne { keys: vec![0], values: vec![1] },
                    ),
                    ty => (ty.clone(), ir::Mode::ExactlyOne { key: 0, values: vec![1] }),
                };
                p.relation(
                    ir::Relation::new(
                        format!("{identity}.{}", field.name),
                        if word(&block.line.tokens, 0) == "derived" { ir::RelationKind::Derived } else { ir::RelationKind::State },
                        [ir::Slot::new(name, ir::Type::entity(identity.clone())), ir::Slot::new(field.name.clone(), ty)],
                    )
                    .mode(mode)
                    .at(self.span(block.line.no)),
                );
            }
        }
        for (mut effect, shape) in effects {
            if let Some(receipt) = &shape.settled_by {
                let line = top.iter().find(|b| b.line.no == shape.line)
                    .and_then(|b| b.children.iter().rev().find(|b| word(&b.line.tokens, 0) == "settled" && word(&b.line.tokens, 1) == "by"))
                    .map_or(shape.line, |b| b.line.no);
                match v
                    .shapes
                    .iter()
                    .position(|s| &s.name == receipt && s.kind == Kind::Event)
                {
                    Some(event) => {
                        let receipt_shape = &v.shapes[event];
                        for slot in &shape.slots {
                            let ty = self.ty(&slot.ty, shape.line);
                            match receipt_shape.slots.iter().find(|s| s.name == slot.name) {
                                Some(received) if self.ty(&received.ty, receipt_shape.line) != ty => self.diagnose(
                                    line,
                                    format!("`settled by {receipt}` has a different type for slot `{}`", slot.name),
                                ),
                                None if matches!(ty, ir::Type::Entity(_)) => self.diagnose(
                                    line,
                                    format!("`settled by {receipt}` lacks entity slot `{}`", slot.name),
                                ),
                                _ => {}
                            }
                        }
                        effect = effect.settled_by(self.ids[event]);
                    }
                    None => self.diagnose(
                        line,
                        format!("`settled by {receipt}` names no event shape"),
                    ),
                }
            }
            p.effect(effect);
        }
        for block in top.iter().filter(|b| word(&b.line.tokens, 0) == "define") {
            if let Some(definition) = self.definition(block) {
                p.definition(definition);
            }
        }
        let known = p.clone().build();
        let parsed = self.parsed;
        for fact in parsed.entity_facts.iter().chain(&parsed.facts) {
            let shape = &v.shapes[fact.shape];
            let values = shape
                .slots
                .iter()
                .zip(&fact.args)
                .map(|(slot, arg)| self.value(arg, &slot.ty, fact.line, &known))
                .collect::<Vec<_>>();
            let fact_ir = ir::Fact::new(self.ids[fact.shape], values).at(self.span(fact.line));
            if shape.kind == Kind::State {
                p.initially(fact_ir);
            } else {
                p.fact(fact_ir);
            }
            self.read(fact.line, Role::Fact, fact.shape, Some(&fact.args));
        }
        for rule in &self.parsed.rules {
            let head_line = heads.get(&rule.line).copied().unwrap_or(rule.line);
            self.clashes(rule, head_line);
            let (head, head_hoisted) = self.framed(|this| this.atom(&rule.head, head_line));
            self.read(
                head_line,
                Role::Head {
                    rule: rule.name.clone(),
                },
                rule.head.shape,
                Some(&rule.head.args),
            );
            let context = Context::Rule(rule.name.clone());
            let mut body = Vec::new();
            let mut body_sources = Vec::new();
            for (line, literal) in &rule.body {
                let (literal, hoisted) = self.framed(|this| this.literal(literal, *line, &context));
                body_sources.extend(hoisted.iter().map(|_| self.span(*line)));
                body.extend(hoisted);
                body.push(literal);
                body_sources.push(self.span(*line));
            }
            body_sources.extend(head_hoisted.iter().map(|_| self.span(head_line)));
            body.extend(head_hoisted);
            let shape = &v.shapes[rule.head.shape];
            if shape.template.is_empty() && !shape.modes[0].exactly {
                body.push(ir::Literal::compare(ir::CompareOp::NotEqual, head.arguments[1].clone(), ir::Expr::value(ir::Value::Null)));
            }
            let mut lowered = ir::Rule::new(qualify(&self.module, &rule.name), head)
                .when(body)
                .at(self.span(rule.line));
            lowered.body_sources = body_sources;
            p.rule(lowered);
        }
        let mut handlers: Vec<ir::Handler> = Vec::new();
        for handler in &self.parsed.handlers {
            let (clauses, event) = self.handler(handler, &link.entity_fields);
            let mut clauses = clauses.into_iter();
            let clause = clauses.next().unwrap();
            match handlers.iter_mut().find(|h| h.event.relation == event.relation) {
                Some(earlier) => {
                    if earlier.may_skip != handler.may_skip {
                        let first = earlier.clauses[0].source.as_ref().unwrap();
                        self.diagnose(handler.line, format!("clauses of event `{}` must all say `may skip` or none does: clause at {}:{} {} it, but clause at {}:{} {} it",
                            self.parsed.vocabulary.shapes[handler.event.shape].name,
                            first.file, first.line, if earlier.may_skip { "says" } else { "omits" },
                            self.span(handler.line).file, self.span(handler.line).line,
                            if handler.may_skip { "says" } else { "omits" }));
                    }
                    earlier.clauses.push(clause);
                    earlier.clauses.extend(clauses);
                }
                None => {
                    let mut lowered = ir::Handler::new(event).clause(clause);
                    lowered.clauses.extend(clauses);
                    lowered.may_skip = handler.may_skip;
                    handlers.push(lowered);
                }
            }
        }
        for handler in handlers {
            p.handler(handler);
        }
        for block in &self.parsed.requirements {
            for requirement in &block.requirements {
                let Some(claim) = self.claim(&requirement.claim, requirement.line) else {
                    continue;
                };
                let text = self.lines.get(&requirement.line).map_or(String::new(), |l| {
                    l.text.trim().trim_start_matches("invariant ").to_string()
                });
                let mut lowered = ir::Requirement::new(text, claim).at(self.span(requirement.line));
                lowered.invariant = requirement.invariant;
                p.requirement(lowered);
            }
        }
        let results: BTreeMap<String, ir::Type> =
            p.definitions().iter().map(|d| (d.name.clone(), d.result.clone())).collect();
        for block in top.iter().filter(|b| word(&b.line.tokens, 0) == "view") {
            if let Some(view) = self.view(block, &results) {
                p.view(view);
            }
        }
    }

    fn fields(&mut self, block: &Block) -> Vec<ir::Slot> {
        let mut fields = Vec::new();
        for child in &block.children {
            let tokens = &child.line.tokens;
            match tokens.as_slice() {
                [name, colon, ty @ ..] if is_word(name) && is_symbol(colon, ":") && !ty.is_empty() => {
                    let ty = self.type_of(ty, child.line.no);
                    fields.push(ir::Slot::new(word(tokens, 0), ty));
                }
                _ => self.diagnose(child.line.no, "an entity field is `name: Type`"),
            }
        }
        fields
    }

    fn type_of(&mut self, tokens: &[Token], line: usize) -> ir::Type {
        match tokens {
            [first, rest @ ..] if word(std::slice::from_ref(first), 0) == "maybe" && !rest.is_empty() => {
                self.type_of(rest, line).maybe()
            }
            _ => {
                let text = (0..tokens.len()).map(|i| word(tokens, i)).collect::<Vec<_>>().join(" ");
                self.ty(&text, line)
            }
        }
    }

    fn parameters(&mut self, tokens: &[Token], line: usize) -> Vec<ir::Slot> {
        tokens
            .split(|t| is_symbol(t, ","))
            .filter(|p| !p.is_empty())
            .filter_map(|param| match param {
                [name, colon, ty @ ..] if is_word(name) && is_symbol(colon, ":") && !ty.is_empty() => {
                    Some(ir::Slot::new(word(param, 0), self.type_of(ty, line)))
                }
                _ => {
                    self.diagnose(line, "a parameter is `name: Type`");
                    None
                }
            })
            .collect()
    }

    fn body_expr(&mut self, tokens: &[Token], line: &Line) -> Option<Expr> {
        match parse_expr(tokens, line) {
            Ok(e) => Some(e),
            Err(error) => {
                self.diagnostics.push(error);
                None
            }
        }
    }

    fn definition(&mut self, block: &Block) -> Option<ir::Definition> {
        let line = &block.line;
        if block.children.is_empty() {
            return None;
        }
        let signatures = self.signatures;
        let signature = signatures.iter().find(|s| s.line == line.no)?;
        let parameters: Vec<_> = signature
            .params
            .iter()
            .map(|(name, ty)| ir::Slot::new(name.as_str(), self.value_type(ty, line.no)))
            .collect();
        let result = self.value_type(&signature.result, line.no);
        let mut body_tokens = Vec::new();
        let mut stack: Vec<&Block> = block.children.iter().rev().collect();
        while let Some(b) = stack.pop() {
            body_tokens.extend(b.line.tokens.iter().cloned());
            stack.extend(b.children.iter().rev());
        }
        let first = &block.children[0].line;
        let body = self.body_expr(&body_tokens, first)?;
        self.params = parameters.iter().map(|s| s.name.clone()).collect();
        let mut direct = Vec::new();
        let mut nested = Vec::new();
        self.expr_vars(&body, first.no, &mut direct, &mut nested);
        self.scope(&[], direct, nested);
        let body = self.expr(&body, first.no);
        self.params.clear();
        Some(ir::Definition {
            name: qualify(&self.module, &signature.name),
            parameters,
            result,
            body,
            effect: ir::EffectLevel::Total,
            source: Some(self.span(line.no)),
        })
    }

    fn value_type(&mut self, ty: &Type, line: usize) -> ir::Type {
        match ty {
            Type::Maybe(inner) => self.value_type(inner, line).maybe(),
            Type::Nix(inner) => self.value_type(inner, line).nix(),
            Type::Named { name, args, .. } => match (name.as_str(), args.as_slice()) {
                ("List", [inner]) => self.value_type(inner, line).list(),
                ("Attrs", [inner]) => ir::Type::Attrs(Box::new(self.value_type(inner, line))),
                (_, []) => self.ty(name, line),
                _ => {
                    self.diagnose(line, format!("`{name}` has no Core IR type with these arguments"));
                    ir::Type::Any
                }
            },
        }
    }

    fn view(&mut self, block: &Block, results: &BTreeMap<String, ir::Type>) -> Option<ir::View> {
        let line = &block.line;
        let tokens = &line.tokens;
        let well_formed = tokens.len() >= 2
            && is_word(&tokens[1])
            && (tokens.len() == 2
                || (word(tokens, 2) == "for"
                    && ((tokens.len() == 4 && is_word(&tokens[3]))
                        || (tokens.len() > 4
                            && is_symbol(&tokens[3], "{")
                            && is_symbol(&tokens[tokens.len() - 1], "}")))));
        if !well_formed {
            self.diagnose(line.no, "a view is `view Name for Type` or `view Name for {param: Type}` with its fields indented beneath");
            return None;
        }
        let bare = tokens.len() == 4;
        let parameters = if bare {
            let written = word(tokens, 3);
            if !written.chars().next().is_some_and(char::is_uppercase) {
                self.diagnose(line.no, "a bare view slot names a declared type beginning with a capital");
                return None;
            }
            let ty = match self.parsed.vocabulary.resolve_type(line, written) {
                Ok(ty) if self.parsed.vocabulary.types.contains_key(&ty) => ty,
                Ok(_) => {
                    self.diagnose(line.no, format!("a prelude type needs a named slot; write `{{name: {written}}}`"));
                    return None;
                }
                Err(error) => {
                    self.diagnostics.push(error);
                    return None;
                }
            };
            let name: String = written.chars().enumerate().flat_map(|(at, c)| {
                let prefix = if at > 0 && c.is_uppercase() { "-" } else { "" };
                prefix.chars().chain(c.to_lowercase())
            }).collect();
            self.view_params.insert(name.clone(), ty.clone());
            vec![ir::Slot::new(name, self.ty(&ty, line.no))]
        } else if tokens.len() > 2 {
            self.parameters(&tokens[4..tokens.len() - 1], line.no)
        } else {
            Vec::new()
        };
        if tokens.len() > 2 && !bare {
            self.view_params = tokens[4..tokens.len() - 1]
                .split(|t| is_symbol(t, ","))
                .filter_map(|param| match param {
                    [name, colon, ty @ ..] if is_word(name) && is_symbol(colon, ":") && !ty.is_empty() => {
                        let ty = (0..ty.len()).map(|i| word(ty, i)).collect::<Vec<_>>().join(" ");
                        Some((word(param, 0).to_string(), ty))
                    }
                    _ => None,
                })
                .collect();
        }
        let mut fields = Vec::new();
        for child in &block.children {
            let l = &child.line;
            match l.tokens.as_slice() {
                [name, colon, rest @ ..] if is_word(name) && is_symbol(colon, ":") && !rest.is_empty() => {
                    if let Some(e) = self.body_expr(rest, l) {
                        let mut direct = self.view_params.keys().map(|name| (name.clone(), line.no)).collect();
                        let mut nested = Vec::new();
                        self.expr_vars(&e, l.no, &mut direct, &mut nested);
                        let before = self.diagnostics.len();
                        self.scope(&[], direct, nested);
                        let value = self.expr(&e, l.no);
                        let name = word(&l.tokens, 0).to_string();
                        let types = parameters.iter().map(|slot| (slot.name.clone(), slot.ty.clone())).collect();
                        match self.typed(&e, &types, results, l) {
                            Ok(ty) => fields.push((ir::Slot::new(name, ty), value)),
                            Err(why) if self.diagnostics.len() == before => self.diagnose(l.no, format!("view field `{name}` has no type: {why}")),
                            Err(_) => {}
                        }
                    }
                }
                _ => self.diagnose(l.no, "a view field is `name: expression`"),
            }
        }
        self.view_params.clear();
        Some(ir::View {
            name: qualify(&self.module, word(tokens, 1)),
            parameters,
            fields,
            source: Some(self.span(line.no)),
        })
    }

    fn bind_types(
        &mut self,
        body: &[Literal],
        types: &mut BTreeMap<String, ir::Type>,
        results: &BTreeMap<String, ir::Type>,
        line: &Line,
    ) {
        for literal in body {
            if let Literal::Sentence(reading) = literal {
                let slots = self.parsed.vocabulary.shapes[reading.shape].slots.clone();
                for (arg, slot) in reading.args.iter().zip(&slots) {
                    if let Arg::Var(name) = arg {
                        if name != "_" && !types.contains_key(name) {
                            let ty = self.ty(&slot.ty, line.no);
                            types.insert(name.clone(), ty);
                        }
                    }
                }
            }
        }
        for literal in body {
            let found = match literal {
                Literal::Bind(var, e) => self.typed(e, types, results, line).ok().map(|ty| (var, ty)),
                Literal::In(var, e) => match self.typed(e, types, results, line) {
                    Ok(ir::Type::List(inner)) => Some((var, *inner)),
                    _ => None,
                },
                Literal::Aggregate { var, op, of, .. } => match (op, of) {
                    (crate::Aggregate::Min | crate::Aggregate::Max, Some(of)) => {
                        self.typed(of, types, results, line).ok().map(|ty| (var, ty))
                    }
                    _ => Some((var, ir::Type::Int)),
                },
                Literal::ArgMin { var, of, body, .. } => {
                    let mut inner = types.clone();
                    self.bind_types(body, &mut inner, results, line);
                    inner.get(of).cloned().map(|ty| (var, ty))
                }
                _ => None,
            };
            if let Some((var, ty)) = found {
                types.entry(var.clone()).or_insert(ty);
            }
        }
    }

    fn typed(
        &mut self,
        e: &Expr,
        types: &BTreeMap<String, ir::Type>,
        results: &BTreeMap<String, ir::Type>,
        line: &Line,
    ) -> Result<ir::Type, String> {
        Ok(match &e.kind {
            ExprKind::Bool(_) => ir::Type::Bool,
            ExprKind::Int(_) | ExprKind::Neg(_) => ir::Type::Int,
            ExprKind::Text(_) | ExprKind::TextBlock(_) => ir::Type::Text,
            ExprKind::Path(_) => ir::Type::Path,
            ExprKind::Not(_) => ir::Type::Bool,
            ExprKind::Var(name) => types.get(name).cloned().ok_or_else(|| format!("`?{name}` has no type here"))?,
            ExprKind::Name(n) => match (self.known.get(n), self.parsed.vocabulary.entities.get(n)) {
                (Some(value), _) => value_ty(value).ok_or_else(|| format!("`{n}` has no type"))?,
                (None, Some(declared)) if declared.len() == 1 => {
                    let ty = declared.first().unwrap().clone();
                    self.ty(&ty, line.no)
                }
                _ => return Err(format!("`{n}` has no type here")),
            },
            ExprKind::List(items) => match items.first() {
                Some(first) => self.typed(first, types, results, line)?.list(),
                None => return Err("an empty list has no element type".into()),
            },
            ExprKind::Record(fields) => {
                let mut out = BTreeMap::new();
                for field in fields {
                    let [Segment::Name(name) | Segment::Text(name)] = field.path.segments.as_slice() else {
                        return Err("a record field has one plain name".into());
                    };
                    out.insert(name.clone(), self.typed(&field.value, types, results, line)?);
                }
                ir::Type::Record(out)
            }
            ExprKind::Field(value, field) => match self.typed(value, types, results, line)? {
                ir::Type::Entity(entity) => {
                    let fields = &self.parsed.vocabulary.fields;
                    match fields.get(&(entity.clone(), field.clone())) {
                        Some(ty) => {
                            let ty = ty.clone();
                            self.ty(&ty, line.no)
                        }
                        None => return Err(no_field(&entity, field, fields.keys().filter(|(t, _)| *t == entity).map(|(_, f)| f.as_str()))),
                    }
                }
                ir::Type::Record(fields) => match fields.get(field) {
                    Some(ty) => ty.clone(),
                    None => return Err(no_field("the record", field, fields.keys().map(String::as_str))),
                },
                other => return Err(format!("`{other:?}` has no field `{field}`")),
            },
            ExprKind::Call(function, _) => match &function.kind {
                ExprKind::Name(name) => match self.calls.get(name).and_then(|q| results.get(q.first()?)) {
                    Some(ty) => ty.clone(),
                    None => match name.as_str() {
                        "length" => ir::Type::Int,
                        "contains" | "starts-with" => ir::Type::Bool,
                        "lowercase" | "trim" | "first-word" | "rest-words" => ir::Type::Text,
                        _ => return Err(format!("`{name}` has no known result type")),
                    },
                },
                _ => return Err("a computed function has no known result type".into()),
            },
            ExprKind::If(_, then_value, else_value) => match (&then_value.kind, &else_value.kind) {
                (ExprKind::Null, _) => self.typed(else_value, types, results, line)?.maybe(),
                (_, ExprKind::Null) => self.typed(then_value, types, results, line)?.maybe(),
                _ => self.typed(then_value, types, results, line)?,
            },
            ExprKind::Binary(op, left, _) => match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul => ir::Type::Int,
                BinOp::Concat | BinOp::Update => self.typed(left, types, results, line)?,
                _ => ir::Type::Bool,
            },
            ExprKind::Query(query) => {
                let body = self.parsed.vocabulary.read_query(line, query).map_err(|e| e.message)?;
                let mut inner = types.clone();
                self.bind_types(&body, &mut inner, results, line);
                match query {
                    Query::The { var, .. } => {
                        let owner = crate::check::Owner { outside: types.keys().cloned().collect(), types: BTreeMap::new() };
                        let fixed = crate::check::the_type(&self.parsed.vocabulary, &owner, var, &body)
                            .ok_or_else(|| format!("no chain of `exactly one` or `at most one` modes fixes `?{var}`"))?;
                        let ty = inner.get(var).cloned().ok_or_else(|| format!("`?{var}` has no type here"))?;
                        match (fixed, &ty) {
                            (crate::check::Ty::Maybe(_), ir::Type::Maybe(_)) | (crate::check::Ty::Maybe(_), ir::Type::Any) => ty,
                            (crate::check::Ty::Maybe(_), _) => ty.maybe(),
                            _ => ty,
                        }
                    }
                    Query::All { head: Some(head), .. } => self.typed(head, &inner, results, line)?.list(),
                    Query::All { vars, .. } if vars.len() == 1 => {
                        inner.get(&vars[0]).cloned().ok_or_else(|| format!("`?{}` has no type here", vars[0]))?.list()
                    }
                    Query::All { vars, .. } => {
                        let mut out = BTreeMap::new();
                        for var in vars {
                            out.insert(var.clone(), inner.get(var).cloned().ok_or_else(|| format!("`?{var}` has no type here"))?);
                        }
                        ir::Type::Record(out).list()
                    }
                    Query::Aggregate { op, value, .. } => match (op, value) {
                        (crate::Aggregate::Min | crate::Aggregate::Max, Some(value)) => {
                            self.typed(value, &inner, results, line)?.maybe()
                        }
                        _ => ir::Type::Int,
                    },
                }
            }
            ExprKind::Null => return Err("`null` alone has no type".into()),
            _ => return Err("this expression has no Core IR type".into()),
        })
    }

    fn handler(
        &mut self,
        handler: &crate::Handler,
        entity_fields: &BTreeMap<String, Vec<ir::Slot>>,
    ) -> (Vec<ir::HandlerClause>, ir::Atom) {
        let mut outer = Vec::new();
        let mut nested = Vec::new();
        for arg in &handler.event.args { self.arg_vars(arg, handler.line, &mut outer, &mut nested); }
        for (line, action) in &handler.actions {
            match action {
                Action::Set(reading) | Action::Include(reading) | Action::Withdraw(reading) | Action::Emit(reading) => {
                    for arg in &reading.args { self.arg_vars(arg, *line, &mut outer, &mut nested); }
                }
                Action::Create { var, fields, .. } => {
                    outer.push((var.clone(), *line));
                    for (_, value) in fields { self.expr_vars(value, *line, &mut outer, &mut nested); }
                }
                Action::Delete { var, .. } => outer.push((var.clone(), *line)),
                Action::Every { when, post, .. } => {
                    let mut direct = Vec::new();
                    let mut deeper = Vec::new();
                    match post.as_ref() {
                        Action::Set(r) | Action::Include(r) | Action::Withdraw(r) | Action::Emit(r) => {
                            for arg in &r.args { self.arg_vars(arg, *line, &mut direct, &mut deeper); }
                        }
                        Action::Delete { var, .. } => direct.push((var.clone(), *line)),
                        Action::Create { .. } | Action::Every { .. } => {}
                    }
                    nested.push(Scope {
                        declared: Vec::new(),
                        head: None,
                        line: *line,
                        body: when.iter().map(|l| (*line, l.clone())).collect(),
                        direct,
                        nested: deeper,
                    });
                }
            }
        }
        self.scope(&handler.when, outer, nested);
        self.handler_args = handler.event.args.iter().enumerate().filter_map(|(slot, arg)| match arg {
                Arg::Var(name) => Some((name.clone(), format!("@event{slot}"))),
                _ => None,
            }).collect();
        let event = self.atom(&handler.event, handler.line);
        self.read(handler.line, Role::Event, handler.event.shape, Some(&handler.event.args));
        let context = Context::Handler(self.parsed.vocabulary.shapes[handler.event.shape].name.clone());
        let mut clause = ir::HandlerClause::new().at(self.span(handler.line));
        for (line, literal) in &handler.when {
            let (literal, hoisted) = self.framed(|this| this.literal(literal, *line, &context));
            clause.body_sources.extend(hoisted.iter().map(|_| self.span(*line)));
            clause.body.extend(hoisted);
            clause.body.push(literal);
            clause.body_sources.push(self.span(*line));
        }
        self.hoist = Some(Vec::new());
        let mut every = Vec::new();
        for (line, action) in &handler.actions {
            let line = *line;
            if let Action::Every { when, post, .. } = action {
                every.push((line, when, post.as_ref()));
                continue;
            }
            for literal in self.hoist.replace(Vec::new()).unwrap_or_default() {
                clause.body.push(literal);
                clause.body_sources.push(self.span(line));
            }
            let sentence = |this: &mut Self, r: &Reading| {
                this.read(line, this.role(&context, false), r.shape, Some(&r.args));
                this.atom(r, line)
            };
            let lowered = match action {
                Action::Set(r) => ir::Action::Set(sentence(self, r)),
                Action::Include(r) => ir::Action::Include(sentence(self, r)),
                Action::Withdraw(r) => ir::Action::Withdraw(sentence(self, r)),
                Action::Emit(r) => ir::Action::Emit(sentence(self, r)),
                Action::Create { var, ty, fields } => {
                    let identity = self.parsed.vocabulary.canonical(&ty);
                    let mut declared = entity_fields.get(&identity).cloned().unwrap_or_default();
                    for shape in &self.parsed.vocabulary.shapes {
                        if shape.template.is_empty() && shape.kind == Kind::State
                            && self.parsed.vocabulary.canonical(&shape.slots[0].ty) == identity
                        {
                            let field = &shape.slots[1];
                            let field_type = &self.parsed.vocabulary.fields[&(identity.clone(), field.name.clone())];
                            declared.push(ir::Slot::new(&field.name, self.ty(field_type, line)));
                        }
                    }
                    for (name, _) in fields {
                        if !declared.iter().any(|slot| &slot.name == name) {
                            self.diagnose(line, format!("`{ty}` has no field `{name}`"));
                        }
                    }
                    let missing: Vec<String> = declared
                        .iter()
                        .filter(|slot| !matches!(slot.ty, ir::Type::Maybe(_)))
                        .filter(|slot| !fields.iter().any(|(name, _)| name == &slot.name))
                        .map(|slot| format!("`{}`", slot.name))
                        .collect();
                    if !missing.is_empty() {
                        self.diagnose(
                            line,
                            format!(
                                "`?{var} is a new {ty}` gives every field whose type is not `maybe`; add {}",
                                missing.join(", ")
                            ),
                        );
                    }
                    clause.actions.push(ir::Action::Create {
                        variable: self.variable(var, line),
                        entity_type: identity,
                        fields: fields
                            .iter()
                            .filter(|(name, e)| {
                                !matches!(e.kind, ExprKind::Null)
                                    || !declared.iter().any(|slot| slot.name == *name && matches!(slot.ty, ir::Type::Maybe(_)))
                            })
                            .map(|(name, e)| (name.clone(), self.expr(e, line)))
                            .collect(),
                    });
                    match self.membership(ty, var) {
                        Some(atom) => ir::Action::Include(atom),
                        None => continue,
                    }
                }
                Action::Delete { .. } | Action::Every { .. } => {
                    self.post(action, line, &context, &mut clause);
                    continue;
                }
            };
            clause.actions.push(lowered);
            for literal in self.hoist.replace(Vec::new()).unwrap_or_default() {
                clause.body.push(literal);
                clause.body_sources.push(self.span(line));
            }
        }
        for literal in self.hoist.take().unwrap_or_default() {
            clause.body.push(literal);
            clause.body_sources.push(self.span(handler.line));
        }
        let mut clauses = Vec::new();
        for (line, when, post) in every {
            let mut extra = ir::HandlerClause::new().at(self.span(line));
            extra.body = clause.body.clone();
            extra.body_sources = clause.body_sources.clone();
            for literal in when {
                let (literal, hoisted) = self.framed(|this| this.literal(literal, line, &context));
                extra.body_sources.extend(hoisted.iter().map(|_| self.span(line)));
                extra.body.extend(hoisted);
                extra.body.push(literal);
                extra.body_sources.push(self.span(line));
            }
            let ((), hoisted) = self.framed(|this| this.post(post, line, &context, &mut extra));
            extra.body_sources.extend(hoisted.iter().map(|_| self.span(line)));
            extra.body.extend(hoisted);
            clauses.push(extra);
        }
        let mut outer: BTreeSet<_> = free_variables(&[event.clone().positive()]).into_iter().collect();
        outer.extend(free_variables(&clause.body));
        for literal in &clause.body {
            match literal {
                ir::Literal::Bind { variable, .. } | ir::Literal::Member { variable, .. }
                | ir::Literal::Aggregate { variable, .. } | ir::Literal::ArgMin { variable, .. } => {
                    outer.insert(variable.clone());
                }
                _ => {}
            }
        }
        for action in &clause.actions {
            if let ir::Action::Create { variable, .. } = action {
                outer.insert(variable.clone());
            }
        }
        for (line, claim) in &handler.ensures {
            let lowered = self.state_claim(claim, *line);
            if self.claim_bound(&ir::Claim::State(lowered.clone()), &outer, *line) {
                clause.ensures.push(lowered);
            }
        }
        self.handler_args.clear();
        clauses.insert(0, clause);
        (clauses, event)
    }

    fn post(&mut self, action: &Action, line: usize, context: &Context, clause: &mut ir::HandlerClause) {
        let sentence = |this: &mut Self, r: &Reading| {
            this.read(line, this.role(context, false), r.shape, Some(&r.args));
            this.atom(r, line)
        };
        let lowered = match action {
            Action::Set(r) => ir::Action::Set(sentence(self, r)),
            Action::Include(r) => ir::Action::Include(sentence(self, r)),
            Action::Withdraw(r) => ir::Action::Withdraw(sentence(self, r)),
            Action::Emit(r) => ir::Action::Emit(sentence(self, r)),
            Action::Delete { var, ty } => {
                clause.actions.push(ir::Action::Delete(ir::Expr::Variable(self.variable(var, line))));
                match self.membership(ty, var) {
                    Some(atom) => ir::Action::Withdraw(atom),
                    None => return,
                }
            }
            Action::Create { .. } | Action::Every { .. } => return,
        };
        clause.actions.push(lowered);
    }

    fn membership(&self, ty: &str, var: &str) -> Option<ir::Atom> {
        let shape = self.parsed.vocabulary.entity_shape(ty)?;
        (self.parsed.vocabulary.shapes[shape].kind == Kind::State).then_some(())?;
        Some(ir::Atom::new(self.ids[shape], vec![ir::Expr::var(self.handler_args.get(var).map_or(var, String::as_str))]))
    }

    fn claim(&mut self, claim: &Claim, line: usize) -> Option<ir::Claim> {
        let lowered = self.lower_claim(claim, line)?;
        self.claim_bound(&lowered, &BTreeSet::new(), line).then_some(lowered)
    }

    fn claim_bound(&mut self, claim: &ir::Claim, outer: &BTreeSet<ir::Variable>, line: usize) -> bool {
        let variables = ir::claims::unbound(claim, outer);
        if variables.is_empty() {
            return true;
        }
        let mut subject = claim;
        while let ir::Claim::NoSinglePointOfFailure { claim, .. } = subject {
            subject = claim;
        }
        let form = match subject {
            ir::Claim::Always(_) => Some("always"),
            ir::Claim::Eventually(_) => Some("possibly"),
            _ => None,
        };
        self.diagnose(line, ir::claims::ClaimError::Unquantified {
            requirement: "claim".to_string(),
            variables: variables.into_iter().collect(),
            form,
            source: None,
        }.to_string());
        false
    }

    fn lower_claim(&mut self, claim: &Claim, line: usize) -> Option<ir::Claim> {
        let diagnostics = self.diagnostics.len();
        let lowered = match claim {
            Claim::State(_) | Claim::Quantified { .. } | Claim::Before(_) => {
                ir::Claim::State(self.state_claim(claim, line))
            }
            Claim::Eventually(s) => ir::Claim::Eventually(self.sentences(s, line)),
            Claim::Always(s) => ir::Claim::Always(self.sentences(s, line)),
            Claim::Never(s) => {
                let mut state = self.sentences(s, line);
                let variables = ir::claims::unbound(&ir::Claim::State(state.clone()), &BTreeSet::new());
                if let ir::StateClaim::Some { variables: existential, .. } = &mut state {
                    existential.extend(variables);
                }
                ir::Claim::Never(state)
            }
            Claim::Requires(target, condition) => ir::Claim::Requires {
                target: self.single(target, line)?,
                condition: self.single(condition, line)?,
            },
            Claim::NoSinglePointOfFailure(inner, among) => ir::Claim::NoSinglePointOfFailure {
                claim: Box::new(match inner.as_ref() {
                    Claim::State(body) => ir::Claim::State(ir::StateClaim::Holds(self.single(body, line)?)),
                    claim => self.lower_claim(claim, line)?,
                }),
                among: self.single(among, line)?,
            },
            Claim::TracePasses(target, checkpoint) => {
                let target = self.literals(target, line);
                let variables = ir::claims::unbound(
                    &ir::Claim::State(ir::StateClaim::Some { variables: Vec::new(), body: target.clone() }),
                    &BTreeSet::new(),
                ).into_iter().collect();
                let target = match target.as_slice() {
                    [ir::Literal::Positive(atom)] => ir::StateClaim::Holds(atom.clone()),
                    _ => ir::StateClaim::Some { variables: Vec::new(), body: target },
                };
                let checkpoint = self.sentences(checkpoint, line);
                ir::Claim::EveryTracePasses { variables, target, checkpoint }
            }
            Claim::HappensWhen(event, condition) => {
                self.read(line, Role::Claim { negated: false }, event.shape, Some(&event.args));
                let event = self.atom(event, line);
                let condition = self.sentences(condition, line);
                ir::Claim::EveryEventWhen { event, condition }
            }
        };
        (self.diagnostics.len() == diagnostics).then_some(lowered)
    }

    fn state_claim(&mut self, claim: &Claim, line: usize) -> ir::StateClaim {
        match claim {
            Claim::State(s) => self.sentences(s, line),
            Claim::Before(s) => ir::StateClaim::Before(Box::new(self.sentences(s, line))),
            Claim::Quantified {
                quantifier,
                vars,
                range,
                body,
            } => {
                let outer = vars.iter().map(|name| (name.clone(), line)).collect();
                let literals = range.iter().chain(body).map(|literal| (line, literal.clone())).collect::<Vec<_>>();
                self.scope(&literals, outer, Vec::new());
                let variables = vars.iter().map(|var| self.variable(var, line)).collect();
                let range = self.literals(range, line);
                let body = self.literals(body, line);
                match quantifier {
                    Quantifier::Every => ir::StateClaim::Every {
                        variables,
                        domain: range,
                        body,
                    },
                    Quantifier::Some | Quantifier::No => {
                        let body = range.into_iter().chain(body).collect();
                        if *quantifier == Quantifier::Some {
                            ir::StateClaim::Some { variables, body }
                        } else {
                            ir::StateClaim::No { variables, body }
                        }
                    }
                }
            }
            _ => {
                self.diagnose(line, "a temporal claim isn't a state claim");
                ir::StateClaim::Some {
                    variables: Vec::new(),
                    body: Vec::new(),
                }
            }
        }
    }

    fn literals(&mut self, literals: &[Literal], line: usize) -> Vec<ir::Literal> {
        self.scope(&literals.iter().map(|literal| (line, literal.clone())).collect::<Vec<_>>(), Vec::new(), Vec::new());
        literals
            .iter()
            .map(|l| self.literal(l, line, &Context::Claim))
            .collect()
    }

    fn sentences(&mut self, literals: &[Literal], line: usize) -> ir::StateClaim {
        let mut body = self.literals(literals, line);
        match body.as_slice() {
            [ir::Literal::Positive(atom)] if atom.arguments.iter().all(|a| matches!(a, ir::Expr::Value(_))) => {
                let Some(ir::Literal::Positive(atom)) = body.pop() else {
                    unreachable!()
                };
                ir::StateClaim::Holds(atom)
            }
            _ => ir::StateClaim::Some {
                variables: Vec::new(),
                body,
            },
        }
    }

    fn single(&mut self, literals: &[Literal], line: usize) -> Option<ir::Atom> {
        match self.literals(literals, line).as_slice() {
            [ir::Literal::Positive(atom)] => Some(atom.clone()),
            _ => {
                self.diagnose(line, "this claim takes one sentence here");
                None
            }
        }
    }

    fn role(&self, context: &Context, negated: bool) -> Role {
        match context {
            Context::Rule(rule) => Role::Condition {
                rule: rule.clone(),
                negated,
            },
            Context::Query => Role::Query,
            Context::Handler(event) => Role::Handler {
                event: event.clone(),
                negated,
            },
            Context::Claim => Role::Claim { negated },
        }
    }

    fn value(&mut self, arg: &Arg, ty: &str, line: usize, known: &ir::Program) -> ir::Value {
        match self.arg(arg, ty, line) {
            ir::Expr::Value(value) => value,
            computed => match ground(computed, known) {
                Some(value) => value,
                None => {
                    self.diagnose(line, format!("`{arg}` isn't a ground value a fact can state"));
                    ir::Value::Null
                }
            },
        }
    }

    fn clashes(&mut self, rule: &crate::Rule, head_line: usize) {
        let mut outer = Vec::new();
        let mut nested = Vec::new();
        for arg in &rule.head.args {
            self.arg_vars(arg, head_line, &mut outer, &mut nested);
        }
        let body: Vec<(usize, Literal)> = rule.body.clone();
        self.scope(&body, outer, nested);
    }

    fn scope(
        &mut self,
        literals: &[(usize, Literal)],
        mut outer: Vec<(String, usize)>,
        mut nested: Vec<Scope>,
    ) {
        for (line, literal) in literals {
            self.literal_vars(literal, *line, &mut outer, &mut nested);
        }
        for inner in nested {
            if let Some((head, names)) = &inner.head {
                if !self.query_locals.iter().any(|(line, expression, _)| *line == inner.line && expression == head) {
                    let mut level = inner.direct.clone();
                    for (line, literal) in &inner.body { self.literal_vars(literal, *line, &mut level, &mut Vec::new()); }
                    let mut read = Vec::new();
                    let mut nested_head = Vec::new();
                    self.expr_vars(head, inner.line, &mut read, &mut nested_head);
                    while let Some(scope) = nested_head.pop() {
                        read.extend(scope.direct);
                        nested_head.extend(scope.nested);
                        for (line, literal) in scope.body { self.literal_vars(&literal, line, &mut read, &mut nested_head); }
                    }
                    let mut names: BTreeSet<_> = names.iter().cloned().collect();
                    names.extend(read.into_iter().map(|(name, _)| name).filter(|name| level.iter().any(|(local, _)| local == name)));
                    let locals: Vec<String> = names.into_iter().filter(|name| !outer.iter().any(|(outside, _)| outside == name)).collect();
                    if locals.is_empty() { self.diagnose(inner.line, "an `all` head reads at least one local variable"); }
                    self.query_locals.push((inner.line, head.clone(), locals));
                }
            }
            for declared in &inner.declared {
                if let Some((_, at)) = outer.iter().find(|(name, _)| name == declared) {
                    self.diagnose(
                        inner.line,
                        format!(
                            "`?{declared}` is declared by the query on line {}, but also appears outside it on line {at}",
                            inner.line
                        ),
                    );
                }
            }
            self.scope(&inner.body, [outer.clone(), inner.direct].concat(), inner.nested);
        }
    }

    fn literal_vars(
        &mut self,
        literal: &Literal,
        line: usize,
        direct: &mut Vec<(String, usize)>,
        nested: &mut Vec<Scope>,
    ) {
        match literal {
            Literal::Sentence(r) | Literal::Not(r) => {
                for arg in &r.args {
                    self.arg_vars(arg, line, direct, nested);
                }
            }
            Literal::Compare(_, left, right) => {
                self.expr_vars(left, line, direct, nested);
                self.expr_vars(right, line, direct, nested);
            }
            Literal::Holds(e) => self.expr_vars(e, line, direct, nested),
            Literal::Bind(var, e) | Literal::In(var, e) => {
                direct.push((var.clone(), line));
                self.expr_vars(e, line, direct, nested);
            }
            Literal::Aggregate { var, of, body, .. } => {
                direct.push((var.clone(), line));
                let mut inner = Vec::new();
                let mut deeper = Vec::new();
                if let Some(e) = of {
                    self.expr_vars(e, line, &mut inner, &mut deeper);
                }
                nested.push(Scope {
                    declared: Vec::new(),
                    head: None,
                    line,
                    body: body.iter().map(|l| (line, l.clone())).collect(),
                    direct: inner,
                    nested: deeper,
                });
            }
            Literal::ArgMin { var, of, by, body, .. } => {
                direct.push((var.clone(), line));
                let mut inner = Vec::new();
                let mut deeper = Vec::new();
                self.expr_vars(by, line, &mut inner, &mut deeper);
                nested.push(Scope {
                    declared: vec![of.clone()],
                    head: None,
                    line,
                    body: body.iter().map(|l| (line, l.clone())).collect(),
                    direct: inner,
                    nested: deeper,
                });
            }
        }
    }

    fn arg_vars(&mut self, arg: &Arg, line: usize, direct: &mut Vec<(String, usize)>, nested: &mut Vec<Scope>) {
        match arg {
            Arg::Var(name) => direct.push((name.clone(), line)),
            Arg::List(items) => {
                for item in items {
                    self.arg_vars(item, line, direct, nested);
                }
            }
            Arg::Expr(text) => self.text_vars(text, line, direct, nested),
            Arg::Text(t) if !t.contains('\n') => self.text_vars(&format!("\"{t}\""), line, direct, nested),
            _ => {}
        }
    }

    fn text_vars(&mut self, text: &str, line: usize, direct: &mut Vec<(String, usize)>, nested: &mut Vec<Scope>) {
        if let Ok(source) = lex(text).map(|mut l| l.remove(0)) {
            if let Ok(e) = parse_expr(&source.tokens, &source) {
                self.expr_vars(&e, line, direct, nested);
            }
        }
    }

    fn expr_vars(&mut self, e: &Expr, line: usize, direct: &mut Vec<(String, usize)>, nested: &mut Vec<Scope>) {
        let mut each = |this: &mut Self, e: &Expr| this.expr_vars(e, line, direct, nested);
        match &e.kind {
            ExprKind::Var(name) => direct.push((name.clone(), line)),
            ExprKind::Text(parts) | ExprKind::TextBlock(parts) => {
                for part in parts {
                    if let TextPart::Interpolation(e) = part {
                        each(self, e);
                    }
                }
            }
            ExprKind::List(items) => items.iter().for_each(|i| each(self, i)),
            ExprKind::Record(fields) => {
                for field in fields {
                    for segment in &field.path.segments { if let Segment::Computed(value) = segment { each(self, value); } }
                    each(self, &field.value);
                }
            }
            ExprKind::Field(value, _) | ExprKind::Select(value, _) | ExprKind::Not(value) | ExprKind::Neg(value) => each(self, value),
            ExprKind::Call(function, arguments) => {
                each(self, function);
                arguments.iter().for_each(|a| each(self, a));
            }
            ExprKind::If(a, b, c) => [a, b, c].into_iter().for_each(|x| each(self, x)),
            ExprKind::Binary(_, a, b) | ExprKind::With(a, b) => {
                each(self, a);
                each(self, b);
            }
            ExprKind::Lambda(params, body) => {
                for param in params {
                    if let Param::Formals { fields, .. } = param {
                        for field in fields {
                            if let Some(default) = &field.default {
                                each(self, default);
                            }
                        }
                    }
                }
                each(self, body);
            }
            ExprKind::Query(query) => {
                let body = match self.lines.get(&line) {
                    Some(&source) => self.parsed.vocabulary.read_query(source, query).unwrap_or_default(),
                    None => Vec::new(),
                };
                let (declared, head, key) = match query {
                    Query::All { vars, head, sorted_by, .. } => (vars.clone(), head.as_deref(), sorted_by.as_deref()),
                    Query::The { var, .. } => (vec![var.clone()], None, None),
                    Query::Aggregate { value, .. } => (Vec::new(), None, value.as_deref()),
                };
                let mut inner = Vec::new();
                let mut deeper = Vec::new();
                let head = head.map(|head| {
                    self.expr_vars(head, line, &mut inner, &mut deeper);
                    let names = inner.iter().map(|(name, _)| name.clone()).collect::<BTreeSet<_>>().into_iter().collect();
                    (head.clone(), names)
                });
                if let Some(key) = key { self.expr_vars(key, line, &mut inner, &mut deeper); }
                nested.push(Scope {
                    declared,
                    head,
                    line,
                    body: body.into_iter().map(|l| (line, l)).collect(),
                    direct: inner,
                    nested: deeper,
                });
            }
            _ => {}
        }
    }

    fn for_queries(&mut self, children: &[Block], outer: &[(String, usize)], out: &mut Vec<(String, usize, ir::Query)>) {
        for child in children {
            let tokens = &child.line.tokens;
            let mut inside = outer.to_vec();
            if word(tokens, 0) == "for" {
                inside.extend(self.for_query(tokens, &child.line, outer, out).into_iter().map(|var| (var, child.line.no)));
            }
            for at in 1..tokens.len() {
                let sym = |i: usize| tokens.get(i).map_or("", crate::expr::word);
                let opens = matches!(sym(at - 1), "{" | ",") && sym(at) == "for";
                if !opens || !matches!(tokens.get(at + 1).map(|t| &t.kind), Some(TokenKind::Variable(_))) {
                    continue;
                }
                let mut depth = 0i32;
                let end = (at..tokens.len()).find(|&end| {
                    match sym(end) {
                        "(" | "[" | "{" => depth += 1,
                        ")" | "]" | "}" => depth -= 1,
                        ":" => return depth == 0,
                        _ => {}
                    }
                    false
                });
                if let Some(end) = end {
                    self.for_query(&tokens[at..end], &child.line, &inside, out);
                }
            }
            self.for_queries(&child.children, &inside, out);
        }
    }

    /// Registers a `for` header's query under the header's source offset and returns the variables it binds.
    fn for_query(&mut self, tokens: &[Token], line: &Line, outer: &[(String, usize)], out: &mut Vec<(String, usize, ir::Query)>) -> Vec<String> {
        let Ok((vars, expr, _)) = crate::record::for_header(tokens, line) else {
            return Vec::new();
        };
        if let ExprKind::Query(query) = &expr.kind {
            let mut direct = outer.to_vec();
            let mut nested = Vec::new();
            self.expr_vars(&expr, line.no, &mut direct, &mut nested);
            self.scope(&[], direct, nested);
            if let Query::All { vars: listed, head: None, conj, .. } = query {
                if let Ok(body) = self.parsed.vocabulary.condition(line, conj, &mut self.view_params.clone()) {
                    let mut grounded = BTreeSet::new();
                    body.iter().for_each(|literal| crate::handlers::ground(literal, &mut grounded));
                    for var in listed.iter().filter(|var| !grounded.contains(*var)) {
                        self.diagnose(line.no, format!("`?{var}` is listed by this `for`, but its condition doesn't ground it: state `?{var}` in a sentence after `where`"));
                    }
                }
            }
            if let ir::Expr::Query(lowered) = self.query(query, line.no) {
                out.push((String::new(), tokens[0].span.start, *lowered));
            }
        }
        vars
    }

    /// Lowers a parsed query, such as a `nix module`'s `for ?m where …`.
    fn query(&mut self, query: &Query, line: usize) -> ir::Expr {
        let Some(&source) = self.lines.get(&line) else {
            self.diagnose(line, "this query's line is missing");
            return ir::Expr::Value(ir::Value::Null);
        };
        let conj = match query {
            Query::All { conj, .. } | Query::The { conj, .. } | Query::Aggregate { conj, .. } => conj,
        };
        let body = match self.parsed.vocabulary.condition(source, conj, &mut self.view_params.clone()) {
            Ok(body) => body,
            Err(error) => {
                let mut error = error;
                for name in self.view_params.keys() {
                    if conj.iter().any(|token| matches!(&token.kind, TokenKind::Word(word) if word == name)) {
                        error.message.push_str(&format!("; read the view slot `{name}` as `?{name}`"));
                    }
                }
                self.diagnostics.push(error);
                return ir::Expr::Value(ir::Value::Null);
            }
        };
        if let Query::All { head: Some(head), .. } = query {
            if !self.query_locals.iter().any(|(at, expression, _)| *at == line && expression == head.as_ref()) {
                let mut nested = Vec::new();
                let mut direct = Vec::new();
                let query_expr = Expr { kind: ExprKind::Query(query.clone()), span: head.span.clone() };
                self.expr_vars(&query_expr, line, &mut direct, &mut nested);
                direct.extend(self.view_params.keys().map(|name| (name.clone(), line)));
                self.scope(&[], direct, nested);
            }
        }
        let mut body: Vec<ir::Literal> = self.conjunction(&body, line, &Context::Query);
        let mut lowered = match query {
            Query::All {
                vars, head, sorted_by, ..
            } => {
                let mut names = vars.clone();
                if let Some(head) = head {
                    names = self.query_locals.iter().find(|(at, expression, _)| *at == line && expression == head.as_ref()).map(|(_, _, names)| names.clone()).unwrap_or_default();
                    if let ExprKind::Record(fields) = &head.kind {
                        if fields.len() >= 2 && fields.iter().all(|field| matches!((&field.path.segments[..], &field.value.kind), ([Segment::Name(name)], ExprKind::Var(var)) if name == var)) {
                            let mut variables: Vec<_> = fields.iter().filter_map(|field| match &field.value.kind { ExprKind::Var(name) => Some(name.as_str()), _ => None }).collect();
                            variables.sort();
                            let offer = variables.into_iter().map(|name| format!("?{name}")).collect::<Vec<_>>().join(", ");
                            self.diagnose(line, format!("a record head that only names its variables is written `all {offer} where …`"));
                        }
                    }
                }
                let (head, mut hoisted) = self.framed(|this| head.as_ref().map(|head| Box::new(this.expr(head, line))));
                let (key, mut sort_hoisted) = self.framed(|this| sorted_by.as_ref().map(|key| this.expr(key, line)));
                body.append(&mut hoisted);
                body.append(&mut sort_hoisted);
                let mut all = ir::Query::all(names.iter().map(|var| self.variable(var, line)), body);
                all.head = head;
                match key {
                    Some(key) => all.sorted_by(key),
                    None => all,
                }
            }
            Query::The { var, .. } => ir::Query::the(self.variable(var, line), body),
            Query::Aggregate { op, value, .. } => {
                let (value, mut hoisted) = self.framed(|this| value.as_ref().map(|value| this.expr(value, line)));
                body.append(&mut hoisted);
                self.hoisted += 1;
                let variable = ir::Variable::new(format!("#aggregate{}", self.hoisted));
                let literal = ir::Literal::aggregate(variable.clone(), aggregate(*op, value), body);
                if let Some(frame) = self.hoist.as_mut() {
                    frame.push(literal);
                    return ir::Expr::Variable(variable);
                }
                ir::Query::the(variable, [literal])
            }
        };
        lowered.source = Some(self.span(line));
        lowered.into()
    }

    fn framed<T>(&mut self, lower: impl FnOnce(&mut Self) -> T) -> (T, Vec<ir::Literal>) {
        let outer = self.hoist.replace(Vec::new());
        let lowered = lower(self);
        let hoisted = std::mem::replace(&mut self.hoist, outer).unwrap_or_default();
        (lowered, hoisted)
    }

    fn conjunction(&mut self, literals: &[Literal], line: usize, context: &Context) -> Vec<ir::Literal> {
        let mut body = Vec::new();
        for literal in literals {
            let (literal, hoisted) = self.framed(|this| this.literal(literal, line, context));
            body.extend(hoisted);
            body.push(literal);
        }
        body
    }

    fn literal(&mut self, literal: &Literal, line: usize, context: &Context) -> ir::Literal {
        match literal {
            Literal::Sentence(reading) => {
                self.read(line, self.role(context, false), reading.shape, Some(&reading.args));
                self.atom(reading, line).positive()
            }
            Literal::Not(reading) => {
                self.read(line, self.role(context, true), reading.shape, Some(&reading.args));
                self.atom(reading, line).negative()
            }
            Literal::Compare(op, left, right) => {
                ir::Literal::compare(compare(*op), self.expr(left, line), self.expr(right, line))
            }
            Literal::Holds(e) => ir::Literal::Boolean(self.expr(e, line)),
            Literal::Bind(var, e) => ir::Literal::bind(self.variable(var, line), self.expr(e, line)),
            Literal::In(var, e) => ir::Literal::member(self.variable(var, line), self.expr(e, line)),
            Literal::Aggregate { var, op, of, body } => {
                let (value, mut hoisted) = self.framed(|this| of.as_ref().map(|e| this.expr(e, line)));
                let mut body = self.conjunction(body, line, context);
                body.append(&mut hoisted);
                ir::Literal::aggregate(self.variable(var, line), aggregate(*op, value), body)
            }
            Literal::ArgMin { direction, var, of, by, body } => {
                let (key, mut hoisted) = self.framed(|this| this.expr(by, line));
                let mut body = self.conjunction(body, line, context);
                body.append(&mut hoisted);
                let candidate = ir::Expr::Variable(self.variable(of, line));
                ir::Literal::ArgMin {
                    direction: *direction,
                    variable: self.variable(var, line),
                    candidate,
                    key,
                    body,
                }
            }
        }
    }

    fn atom(&mut self, reading: &Reading, line: usize) -> ir::Atom {
        let shape = &self.parsed.vocabulary.shapes[reading.shape];
        let arguments: Vec<ir::Expr> = shape
            .slots
            .iter()
            .zip(&reading.args)
            .map(|(slot, arg)| self.arg(arg, &slot.ty, line))
            .collect();
        ir::Atom::new(self.ids[reading.shape], arguments).at(self.span(line))
    }

    fn arg(&mut self, arg: &Arg, ty: &str, line: usize) -> ir::Expr {
        if let Some(inner) = ty.strip_prefix("maybe ") {
            return self.arg(arg, inner, line);
        }
        let ground_list = matches!(arg, Arg::List(_)) && !arg_has_var(arg);
        if ty.starts_with("nix ") && (ground_list || matches!(arg, Arg::Name(_) | Arg::Int(_) | Arg::Text(_) | Arg::Path(_) | Arg::Expr(_))) {
            return ir::Expr::value(ir::Value::Nix {
                normalized: arg.to_string(),
            });
        }
        match arg {
            Arg::Var(name) if name == "_" => self.anonymous(),
            Arg::Var(name) => ir::Expr::Variable(self.variable(name, line)),
            Arg::Int(n) => ir::Expr::value(*n),
            Arg::Text(t) if t.contains('\n') => ir::Expr::value(t.as_str()),
            Arg::Text(t) => self.source(&format!("\"{t}\""), line),
            Arg::Path(p) => ir::Expr::value(ir::Value::Path(p.clone())),
            Arg::List(items) => {
                let item = crate::sentences::list_item(ty).unwrap_or(ty);
                ir::Expr::List(items.iter().map(|a| self.arg(a, item, line)).collect())
            }
            Arg::Name(n) => match ty {
                "Bool" => ir::Expr::value(n == "true"),
                "OptionPath" => {
                    ir::Expr::value(ir::Value::OptionPath(n.split('.').map(String::from).collect()))
                }
                _ => {
                    let v = &self.parsed.vocabulary;
                    let written = v.written(ty);
                    let identity = v.canonical(ty);
                    if v.types.get(ty).is_some_and(Option::is_none) && !v.entities.get(n).is_some_and(|t| t.contains(ty)) {
                        let names = v.entities.iter().filter(|(_, t)| t.contains(ty)).map(|(name, _)| name.as_str());
                        let message = match crate::check::closest(n, names) {
                            Some(near) => format!("`{n}` isn't a declared {written}; the nearest is `{near}`, and `{n} is a {written}` declares a new one"),
                            None => format!("`{n}` isn't a declared {written}; `{n} is a {written}` declares a new one"),
                        };
                        self.diagnose(line, message);
                    }
                    ir::Expr::value(ir::Value::entity(identity, n.as_str()))
                }
            },
            Arg::Expr(text) => self.source(text, line),
        }
    }

    fn source(&mut self, text: &str, line: usize) -> ir::Expr {
        match lex(text).map(|mut l| l.remove(0)) {
            Ok(source) => match parse_expr(&source.tokens, &source) {
                Ok(e) => self.expr(&e, line),
                Err(error) => {
                    self.diagnose(line, error.message);
                    ir::Expr::Value(ir::Value::Null)
                }
            },
            Err(error) => {
                self.diagnose(line, error.message);
                ir::Expr::Value(ir::Value::Null)
            }
        }
    }

    fn anonymous(&mut self) -> ir::Expr {
        self.anonymous += 1;
        ir::Expr::var(format!("_#{}", self.anonymous))
    }

    fn variable(&mut self, name: &str, line: usize) -> ir::Variable {
        if self.params.contains(name) {
            self.diagnose(
                line,
                format!("`?{name}` reuses definition parameter `{name}`; read the parameter bare as `{name}`"),
            );
        }
        ir::Variable::new(self.handler_args.get(name).map_or(name, String::as_str))
    }

    fn expr(&mut self, e: &Expr, line: usize) -> ir::Expr {
        let each = |this: &mut Self, items: &[Expr]| -> Vec<ir::Expr> {
            items.iter().map(|i| this.expr(i, line)).collect()
        };
        match &e.kind {
            ExprKind::Null => ir::Expr::value(ir::Value::Null),
            ExprKind::Bool(b) => ir::Expr::value(*b),
            ExprKind::Int(n) => ir::Expr::value(*n),
            ExprKind::Path(p) => ir::Expr::value(ir::Value::Path(p.clone())),
            ExprKind::Var(name) if name == "_" => self.anonymous(),
            ExprKind::Var(name) => ir::Expr::Variable(self.variable(name, line)),
            ExprKind::Name(n) if self.known.contains_key(n) => ir::Expr::value(self.known[n].clone()),
            ExprKind::Name(n) if self.params.contains(n) => ir::Expr::var(n.as_str()),
            ExprKind::Name(n) => match self.parsed.vocabulary.entities.get(n) {
                Some(types) if types.len() == 1 => {
                    let ty = types.first().unwrap();
                    let identity = self.parsed.vocabulary.canonical(&ty);
                    ir::Expr::value(ir::Value::entity(identity, n.as_str()))
                }
                Some(types) => {
                    let types: Vec<&str> = types.iter().map(|ty| self.parsed.vocabulary.written(ty)).collect();
                    self.diagnose(line, format!("`{n}` is a declared {}, so here it needs a type", types.join(" and ")));
                    ir::Expr::Value(ir::Value::Null)
                }
                None => {
                    let message = if self.view_params.contains_key(n) {
                        format!("read the view slot `{n}` as `?{n}`")
                    } else {
                        format!("`{n}` isn't a declared entity")
                    };
                    self.diagnose(line, message);
                    ir::Expr::Value(ir::Value::Null)
                }
            },
            ExprKind::Text(parts) | ExprKind::TextBlock(parts) => {
                let mut pieces = parts.iter().map(|part| match part {
                    TextPart::Text(t) => ir::Expr::value(t.as_str()),
                    TextPart::Interpolation(e) => {
                        let value = self.expr(e, line);
                        match ground(value.clone(), &ir::Program::builder().build()) {
                            Some(ir::Value::Int(value)) => ir::Expr::value(value.to_string()),
                            _ => value,
                        }
                    },
                });
                let first = pieces.next().unwrap_or_else(|| ir::Expr::value(""));
                pieces.fold(first, |left, right| {
                    ir::Expr::binary(ir::BinaryOp::Concat, left, right)
                })
            }
            ExprKind::List(items) => ir::Expr::List(each(self, items)),
            ExprKind::Record(fields) => {
                let mut out = BTreeMap::new();
                for field in fields {
                    match field.path.segments.as_slice() {
                        [Segment::Name(name) | Segment::Text(name)] => {
                            let value = self.expr(&field.value, line);
                            out.insert(name.clone(), value);
                        }
                        _ => self.diagnose(line, "a record field in a rule has one plain name"),
                    }
                }
                ir::Expr::Record(out)
            }
            ExprKind::Select(..) => {
                self.diagnose(line, "a quoted or computed key selects only from a Nix value");
                ir::Expr::value(ir::Value::Null)
            }
            ExprKind::Field(value, field) => ir::Expr::Field {
                value: Box::new(self.expr(value, line)),
                field: field.clone(),
            },
            ExprKind::Call(function, arguments) => {
                let arguments = each(self, arguments);
                match &function.kind {
                    ExprKind::Name(name) => match self.calls.get(name).map(Vec::as_slice) {
                        Some([one]) => ir::Expr::call(one.as_str(), arguments),
                        Some(many @ [_, _, ..]) => {
                            self.diagnose(line, format!("`{name}` is defined by {}", many.join(" and ")));
                            ir::Expr::call(name.as_str(), arguments)
                        }
                        _ => ir::Expr::call(name.as_str(), arguments),
                    },
                    _ => ir::Expr::Apply {
                        function: Box::new(self.expr(function, line)),
                        arguments,
                    },
                }
            }
            ExprKind::If(condition, then_value, else_value) => ir::Expr::If {
                condition: Box::new(self.expr(condition, line)),
                then_value: Box::new(self.expr(then_value, line)),
                else_value: Box::new(self.expr(else_value, line)),
            },
            ExprKind::Binary(op, left, right) => {
                let op = match op {
                    BinOp::Or => ir::BinaryOp::Or,
                    BinOp::And => ir::BinaryOp::And,
                    BinOp::Update => ir::BinaryOp::Merge,
                    BinOp::Add => ir::BinaryOp::Add,
                    BinOp::Sub => ir::BinaryOp::Subtract,
                    BinOp::Mul => ir::BinaryOp::Multiply,
                    BinOp::Concat => ir::BinaryOp::Concat,
                    compared => ir::BinaryOp::Compare(compare(*compared)),
                };
                ir::Expr::binary(op, self.expr(left, line), self.expr(right, line))
            }
            ExprKind::Not(value) => ir::Expr::Unary {
                op: ir::UnaryOp::Not,
                value: Box::new(self.expr(value, line)),
            },
            ExprKind::Neg(value) => ir::Expr::binary(
                ir::BinaryOp::Subtract,
                ir::Expr::Value(ir::Value::Int(0)),
                self.expr(value, line),
            ),
            ExprKind::Query(query) => self.query(query, line),
            ExprKind::Computed(_) => {
                self.diagnose(line, "a computed name `{e}` is a Nix binder or reference, so it stands only in a Nix body");
                ir::Expr::Value(ir::Value::Null)
            }
            ExprKind::Lambda(..) | ExprKind::With(..) | ExprKind::NixModule(..) => {
                self.diagnose(line, "functions, `with` and Nix modules are lowered with views and the Nix target, not in rules");
                ir::Expr::Value(ir::Value::Null)
            }
        }
    }

    fn read(&mut self, line: usize, role: Role, shape: usize, args: Option<&[Arg]>) {
        let v = &self.parsed.vocabulary;
        let s = &v.shapes[shape];
        let slots = s
            .slots
            .iter()
            .enumerate()
            .map(|(i, slot)| SlotReading {
                name: slot.name.clone(),
                ty: v.written(&slot.ty).to_string(),
                filler: args.map(|a| a[i].to_string()),
            })
            .collect();
        self.readings.push(SentenceReading {
            line,
            role,
            relation: Some(self.ids[shape]),
            shape: s.name.clone(),
            template: v.shape_text(shape),
            slots,
        });
    }

    fn ty(&mut self, ty: &str, line: usize) -> ir::Type {
        if ty.starts_with("maybe ") || ty.starts_with("List ") || ty.starts_with("Attrs ") {
            match lex(ty).and_then(|lines| crate::types::parse_type(&lines[0].tokens, &lines[0])) {
                Ok(ty) => return self.value_type(&ty, line),
                Err(error) => {
                    self.diagnose(line, error.message);
                    return ir::Type::Any;
                }
            }
        }
        if let Some(inner) = ty.strip_prefix("nix ") {
            return self.ty(inner, line).nix();
        }
        if let Some(inner) = ty.strip_prefix("maybe ") {
            return self.ty(inner, line).maybe();
        }
        if let Some(inner) = crate::sentences::list_item(ty) {
            return self.ty(inner, line).list();
        }
        match ty {
            "Any" => ir::Type::Any,
            "Int" => ir::Type::Int,
            "Text" => ir::Type::Text,
            "Bool" => ir::Type::Bool,
            "Path" => ir::Type::Path,
            "OptionPath" => ir::Type::OptionPath,
            _ => match self.parsed.vocabulary.types.get(ty) {
                Some(Some(union)) => ir::Type::TextUnion(union.clone()),
                Some(None) => {
                    ir::Type::entity(self.parsed.vocabulary.canonical(ty))
                }
                None => {
                    self.diagnose(line, format!("`{ty}` has no Core IR type yet"));
                    ir::Type::Any
                }
            },
        }
    }

    fn span(&self, line: usize) -> ir::SourceSpan {
        let source = self.lines.get(&line);
        let tokens = source.map(|l| l.tokens.as_slice()).unwrap_or_default();
        let (file, local, offset) = place(self.starts, line);
        ir::SourceSpan {
            file: file.into(),
            start: tokens.first().map_or(0, |t| t.span.start - offset),
            end: tokens.last().map_or(0, |t| t.span.end - offset),
            line: local,
            column: source.map_or(1, |l| l.indent + 1),
        }
    }

    fn diagnose(&mut self, line: usize, message: impl Into<String>) {
        let error = match self.lines.get(&line) {
            Some(source) => Error::at(source, message),
            None => Error::bare(message),
        };
        self.diagnostics.push(error);
    }
}

struct Scope {
    head: Option<(Expr, Vec<String>)>,
    declared: Vec<String>,
    line: usize,
    body: Vec<(usize, Literal)>,
    direct: Vec<(String, usize)>,
    nested: Vec<Scope>,
}

pub(crate) fn known_expr(e: &Expr, bound: &BTreeMap<String, Option<ir::Value>>) -> Option<ir::Value> {
    let parsed = Sentences::default();
    let mut lowering = Lowering {
        parsed: &parsed,
        signatures: &[],
        lines: BTreeMap::new(),
        starts: &[],
        readings: Vec::new(),
        diagnostics: Vec::new(),
        params: BTreeSet::new(),
        view_params: BTreeMap::new(),
        handler_args: BTreeMap::new(),
        anonymous: 0,
        hoist: None,
        hoisted: 0,
        query_locals: Vec::new(),
        known: bound.iter().filter_map(|(name, value)| value.clone().map(|value| (name.clone(), value))).collect(),
        module: String::new(),
        ids: Vec::new(),
        calls: BTreeMap::new(),
    };
    let value = lowering.expr(e, e.span.line);
    if lowering.diagnostics.is_empty() { ground(value, &ir::Program::builder().build()) } else { None }
}

fn ground(value: ir::Expr, known: &ir::Program) -> Option<ir::Value> {
    galileo_core::answer(
        known,
        &galileo_core::Model::default(),
        &ir::Query::the("@ground", [ir::Literal::bind("@ground", value)]),
        &[],
        galileo_core::Budget::default(),
    ).ok()
}

fn arg_has_var(arg: &Arg) -> bool {
    match arg {
        Arg::Var(_) => true,
        Arg::List(items) => items.iter().any(arg_has_var),
        _ => false,
    }
}

fn kind(kind: Kind) -> ir::RelationKind {
    match kind {
        Kind::Static => ir::RelationKind::Static,
        Kind::State => ir::RelationKind::State,
        Kind::Derived => ir::RelationKind::Derived,
        Kind::Event => ir::RelationKind::Event,
        Kind::Effect => ir::RelationKind::Effect,
    }
}

fn compare(op: BinOp) -> ir::CompareOp {
    match op {
        BinOp::Eq => ir::CompareOp::Equal,
        BinOp::Ne => ir::CompareOp::NotEqual,
        BinOp::Lt => ir::CompareOp::Less,
        BinOp::Le => ir::CompareOp::LessEqual,
        BinOp::Gt => ir::CompareOp::Greater,
        BinOp::In => ir::CompareOp::In,
        _ => ir::CompareOp::GreaterEqual,
    }
}

fn aggregate(op: crate::Aggregate, value: Option<ir::Expr>) -> ir::AggregateOp {
    match (op, value) {
        (crate::Aggregate::Count, _) => ir::AggregateOp::Count,
        (crate::Aggregate::Sum, Some(value)) => ir::AggregateOp::Sum(value),
        (crate::Aggregate::Min, Some(value)) => ir::AggregateOp::Min(value),
        (crate::Aggregate::Max, Some(value)) => ir::AggregateOp::Max(value),
        (_, None) => unreachable!(),
    }
}

fn word(tokens: &[Token], index: usize) -> &str {
    match tokens.get(index).map(|t| &t.kind) {
        Some(TokenKind::Word(w)) => w,
        _ => "",
    }
}

fn is_word(token: &Token) -> bool {
    matches!(token.kind, TokenKind::Word(_))
}

fn is_symbol(token: &Token, text: &str) -> bool {
    matches!(&token.kind, TokenKind::Symbol(s) if s == text)
}

fn free_variables(body: &[ir::Literal]) -> Vec<ir::Variable> {
    let mut out: Vec<ir::Variable> = Vec::new();
    for literal in body {
        if let ir::Literal::Positive(atom) = literal {
            for argument in &atom.arguments {
                if let ir::Expr::Variable(v) = argument {
                    if !out.contains(v) {
                        out.push(v.clone());
                    }
                }
            }
        }
    }
    out
}

fn no_field<'a>(owner: &str, field: &str, candidates: impl IntoIterator<Item = &'a str>) -> String {
    let shown = owner.rsplit('/').next().unwrap_or(owner);
    match crate::check::closest(field, candidates) {
        Some(near) => format!("`{shown}` has no field `{field}`; did you mean `{near}`?"),
        None => format!("`{shown}` has no field `{field}`"),
    }
}

fn value_ty(value: &ir::Value) -> Option<ir::Type> {
    Some(match value {
        ir::Value::Bool(_) => ir::Type::Bool,
        ir::Value::Int(_) => ir::Type::Int,
        ir::Value::Text(_) => ir::Type::Text,
        ir::Value::Path(_) => ir::Type::Path,
        ir::Value::OptionPath(_) => ir::Type::OptionPath,
        ir::Value::Entity(entity) => ir::Type::entity(entity.entity_type.as_str()),
        _ => return None,
    })
}
