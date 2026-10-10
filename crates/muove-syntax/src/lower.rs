use crate::check::{signature, Signature};
use crate::expr::{parse_expr, BinOp, Expr, ExprKind, Param, Query, TextPart};
use crate::record::Segment;
use crate::types::Type;
use crate::{
    blocks, lex, read_program, read_sentences, Action, Arg, Block, Claim, Error, Kind, Line, Literal, Quantifier,
    Reading, Sentences, Token, TokenKind,
};
use muove_core as ir;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct Lowered {
    pub program: ir::Program,
    pub readings: Vec<SentenceReading>,
    pub diagnostics: Vec<Error>,
    /// Each `nix module` or `nix file` `for` block's query, by file and line.
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
    let mut for_queries = Vec::new();
    let mut lines = BTreeMap::new();
    for name in &order {
        let source = modules.iter().find(|(n, _)| n == name).map_or("", |(_, s)| s);
        let starts = [(1, name.clone(), 0)];
        let part = lower_module(&read[name], source, &starts, &mut link).map_err(|e| named(name, e))?;
        diagnostics.extend(part.diagnostics.into_iter().map(|e| named(name, e)));
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
        .filter_map(|b| Some((b.line.no, b.children.first()?.line.no)))
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
        diagnostics: parsed.warnings.clone(),
        params: BTreeSet::new(),
        view_params: BTreeMap::new(),
        handler_args: BTreeMap::new(),
        anonymous: 0,
        module,
        ids: Vec::new(),
        calls,
    };
    lowering.program(&heads, &top, link);
    let mut for_queries = Vec::new();
    for block in top.iter().filter(|b| word(&b.line.tokens, 0) == "nix") {
        lowering.for_queries(&block.children, &mut for_queries);
    }
    Ok(Part {
        readings: lowering.readings,
        diagnostics: lowering.diagnostics,
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
    module: String,
    ids: Vec<ir::RelationId>,
    calls: BTreeMap<String, Vec<String>>,
}

impl Lowering<'_> {
    fn program(&mut self, heads: &BTreeMap<usize, usize>, top: &[Block], link: &mut Link) {
        let v = &self.parsed.vocabulary;
        let p = &mut link.builder;
        for (name, union) in &v.types {
            if union.is_some() || v.type_modules.get(name) != Some(&v.module) || !link.types.insert(name.clone()) {
                continue;
            }
            let record = top.iter().find(|b| {
                matches!(word(&b.line.tokens, 0), "state" | "static")
                    && word(&b.line.tokens, 1) == "entity"
                    && word(&b.line.tokens, 2) == name
            });
            match record {
                Some(block) if word(&block.line.tokens, 0) == "state" => {
                    let fields = self.fields(block);
                    link.entity_fields.insert(name.clone(), fields.clone());
                    p.state_entity_type(name.clone(), fields);
                }
                Some(block) if !block.children.is_empty() => {
                    self.diagnose(block.line.no, "only a `state entity` has fields");
                    p.entity_type(name.clone());
                }
                _ => {
                    p.entity_type(name.clone());
                }
            }
        }
        for fact in &self.parsed.entity_facts {
            let (ty, Some(crate::Arg::Name(name))) = (&v.shapes[fact.shape].slots[0].ty, fact.args.first()) else {
                continue;
            };
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
                            .map(|a| self.value(a, ty, shape.line))
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
            for field in &link.entity_fields[name] {
                let (ty, mode) = match &field.ty {
                    ir::Type::Maybe(ty) => (
                        ty.as_ref().clone(),
                        ir::Mode::AtMostOne { keys: vec![0], values: vec![1] },
                    ),
                    ty => (ty.clone(), ir::Mode::ExactlyOne { key: 0, values: vec![1] }),
                };
                p.relation(
                    ir::Relation::new(
                        qualify(&v.module, &format!("{name}.{}", field.name)),
                        ir::RelationKind::State,
                        [ir::Slot::new(name, ir::Type::entity(name)), ir::Slot::new(field.name.clone(), ty)],
                    )
                    .mode(mode)
                    .at(self.span(block.line.no)),
                );
            }
        }
        for (mut effect, shape) in effects {
            if let Some(receipt) = &shape.settled_by {
                match v
                    .shapes
                    .iter()
                    .position(|s| &s.name == receipt && s.kind == Kind::Event)
                {
                    Some(event) => effect = effect.settled_by(self.ids[event]),
                    None => self.diagnose(
                        shape.line,
                        format!("`settled by {receipt}` names no event shape"),
                    ),
                }
            }
            p.effect(effect);
        }
        let parsed = self.parsed;
        for fact in parsed.entity_facts.iter().chain(&parsed.facts) {
            let shape = &v.shapes[fact.shape];
            let values = shape
                .slots
                .iter()
                .zip(&fact.args)
                .map(|(slot, arg)| self.value(arg, &slot.ty, fact.line))
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
            let head = self.atom(&rule.head, head_line);
            self.read(
                head_line,
                Role::Head {
                    rule: rule.name.clone(),
                },
                rule.head.shape,
                Some(&rule.head.args),
            );
            self.clashes(rule, head_line);
            let mut body = Vec::new();
            let context = Context::Rule(rule.name.clone());
            for (line, literal) in &rule.body {
                body.push(self.literal(literal, *line, &context));
            }
            p.rule(
                ir::Rule::new(qualify(&self.module, &rule.name), head)
                    .when(body)
                    .at(self.span(rule.line)),
            );
        }
        for block in top.iter().filter(|b| word(&b.line.tokens, 0) == "define") {
            if let Some(definition) = self.definition(block) {
                p.definition(definition);
            }
        }
        let mut handlers: Vec<ir::Handler> = Vec::new();
        for handler in &self.parsed.handlers {
            let (clause, event) = self.handler(handler, &link.entity_fields);
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
                }
                None => {
                    let mut lowered = ir::Handler::new(event).clause(clause);
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
                let claim = self.claim(&requirement.claim, requirement.line);
                let text = self.lines.get(&requirement.line).map_or(String::new(), |l| {
                    l.text.trim().trim_start_matches("invariant ").to_string()
                });
                let mut lowered = ir::Requirement::new(text, claim).at(self.span(requirement.line));
                lowered.invariant = requirement.invariant;
                p.requirement(lowered);
            }
        }
        for block in top.iter().filter(|b| word(&b.line.tokens, 0) == "view") {
            if let Some(view) = self.view(block) {
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

    fn view(&mut self, block: &Block) -> Option<ir::View> {
        let line = &block.line;
        let tokens = &line.tokens;
        let well_formed = tokens.len() >= 2
            && is_word(&tokens[1])
            && (tokens.len() == 2
                || (word(tokens, 2) == "for"
                    && tokens.len() > 4
                    && is_symbol(&tokens[3], "{")
                    && is_symbol(&tokens[tokens.len() - 1], "}")));
        if !well_formed {
            self.diagnose(line.no, "a view is `view Name for {param: Type}` with its fields indented beneath");
            return None;
        }
        let parameters = if tokens.len() > 2 {
            self.parameters(&tokens[4..tokens.len() - 1], line.no)
        } else {
            Vec::new()
        };
        if tokens.len() > 2 {
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
                        let value = self.expr(&e, l.no);
                        fields.push((word(&l.tokens, 0).to_string(), value));
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

    fn handler(
        &mut self,
        handler: &crate::Handler,
        entity_fields: &BTreeMap<String, Vec<ir::Slot>>,
    ) -> (ir::HandlerClause, ir::Atom) {
        self.handler_args = handler.event.args.iter().enumerate().filter_map(|(slot, arg)| match arg {
                Arg::Var(name) => Some((name.clone(), format!("@event{slot}"))),
                _ => None,
            }).collect();
        let event = self.atom(&handler.event, handler.line);
        self.read(handler.line, Role::Event, handler.event.shape, Some(&handler.event.args));
        let context = Context::Handler(self.parsed.vocabulary.shapes[handler.event.shape].name.clone());
        let mut clause = ir::HandlerClause::new().at(self.span(handler.line));
        for (line, literal) in &handler.when {
            clause.body.push(self.literal(literal, *line, &context));
        }
        for (line, action) in &handler.actions {
            let line = *line;
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
                    let declared = entity_fields.get(ty).map_or(&[][..], Vec::as_slice);
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
                        entity_type: ty.clone(),
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
                Action::Delete { var, ty } => {
                    clause.actions.push(ir::Action::Delete(ir::Expr::Variable(self.variable(var, line))));
                    match self.membership(ty, var) {
                        Some(atom) => ir::Action::Withdraw(atom),
                        None => continue,
                    }
                }
            };
            clause.actions.push(lowered);
        }
        for (line, claim) in &handler.ensures {
            let lowered = self.state_claim(claim, *line);
            clause.ensures.push(lowered);
        }
        self.handler_args.clear();
        (clause, event)
    }

    fn membership(&self, ty: &str, var: &str) -> Option<ir::Atom> {
        let shape = self.parsed.vocabulary.entity_shape(ty)?;
        (self.parsed.vocabulary.shapes[shape].kind == Kind::State).then_some(())?;
        Some(ir::Atom::new(self.ids[shape], vec![ir::Expr::var(self.handler_args.get(var).map_or(var, String::as_str))]))
    }

    fn claim(&mut self, claim: &Claim, line: usize) -> ir::Claim {
        match claim {
            Claim::State(_) | Claim::Quantified { .. } | Claim::Before(_) => {
                ir::Claim::State(self.state_claim(claim, line))
            }
            Claim::Eventually(s) => ir::Claim::Eventually(self.sentences(s, line)),
            Claim::Always(s) => ir::Claim::Always(self.sentences(s, line)),
            Claim::Never(s) => ir::Claim::Never(self.sentences(s, line)),
            Claim::Requires(target, condition) => ir::Claim::Requires {
                target: self.single(target, line),
                condition: self.single(condition, line),
            },
            Claim::NoSinglePointOfFailure(inner, among) => ir::Claim::NoSinglePointOfFailure {
                claim: Box::new(self.claim(inner, line)),
                among: self.single(among, line),
            },
            Claim::TracePasses(target, checkpoint) => ir::Claim::EveryTracePasses {
                target: self.sentences(target, line),
                checkpoint: self.sentences(checkpoint, line),
            },
            Claim::HappensWhen(event, condition) => {
                self.read(line, Role::Claim { negated: false }, event.shape, Some(&event.args));
                ir::Claim::EveryEventWhen {
                    event: self.atom(event, line),
                    condition: self.sentences(condition, line),
                }
            }
        }
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
                variables: free_variables(&body),
                body,
            },
        }
    }

    fn single(&mut self, literals: &[Literal], line: usize) -> ir::Atom {
        match self.literals(literals, line).as_slice() {
            [ir::Literal::Positive(atom)] => atom.clone(),
            _ => {
                self.diagnose(line, "this claim takes one sentence here");
                ir::Atom::new(ir::RelationId(0), [])
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

    fn value(&mut self, arg: &Arg, ty: &str, line: usize) -> ir::Value {
        match self.arg(arg, ty, line) {
            ir::Expr::Value(value) => value,
            computed => match ground(computed) {
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
            self.scope(&inner.body, [outer.clone(), inner.direct].concat(), Vec::new());
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
                    line,
                    body: body.iter().map(|l| (line, l.clone())).collect(),
                    direct: inner,
                });
                nested.extend(deeper);
            }
            Literal::First { var, of, by, body } => {
                direct.push((var.clone(), line));
                let mut inner = Vec::new();
                let mut deeper = Vec::new();
                self.expr_vars(by, line, &mut inner, &mut deeper);
                nested.push(Scope {
                    declared: vec![of.clone()],
                    line,
                    body: body.iter().map(|l| (line, l.clone())).collect(),
                    direct: inner,
                });
                nested.extend(deeper);
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
            ExprKind::Record(fields) => fields.iter().for_each(|f| each(self, &f.value)),
            ExprKind::Field(value, _) | ExprKind::Not(value) => each(self, value),
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
                let (declared, key) = match query {
                    Query::All { vars, sorted_by, .. } => (vars.clone(), sorted_by.as_deref()),
                    Query::The { var, .. } => (vec![var.clone()], None),
                };
                let mut inner = Vec::new();
                let mut deeper = Vec::new();
                if let Some(key) = key {
                    self.expr_vars(key, line, &mut inner, &mut deeper);
                }
                nested.push(Scope {
                    declared,
                    line,
                    body: body.into_iter().map(|l| (line, l)).collect(),
                    direct: inner,
                });
                nested.extend(deeper);
            }
            _ => {}
        }
    }

    fn for_queries(&mut self, children: &[Block], out: &mut Vec<(String, usize, ir::Query)>) {
        for child in children {
            let tokens = &child.line.tokens;
            if word(tokens, 0) == "for" {
                let source = tokens.iter().position(|t| matches!(&t.kind, TokenKind::Word(w) if w == "in"));
                if let Some(Ok(Expr { kind: ExprKind::Query(query), .. })) =
                    source.map(|split| parse_expr(&tokens[split + 1..], &child.line))
                {
                    if let ir::Expr::Query(lowered) = self.query(&query, child.line.no) {
                        out.push((String::new(), child.line.no, *lowered));
                    }
                }
            }
            self.for_queries(&child.children, out);
        }
    }

    /// Lowers a parsed query, such as a `nix module`'s `for ?m in all ?m where …`.
    fn query(&mut self, query: &Query, line: usize) -> ir::Expr {
        let Some(&source) = self.lines.get(&line) else {
            self.diagnose(line, "this query's line is missing");
            return ir::Expr::Value(ir::Value::Null);
        };
        let conj = match query {
            Query::All { conj, .. } | Query::The { conj, .. } => conj,
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
        let body: Vec<ir::Literal> = body.iter().map(|l| self.literal(l, line, &Context::Query)).collect();
        let lowered = match query {
            Query::All {
                vars, sorted_by, ..
            } => {
                let all = ir::Query::all(vars.iter().map(|var| self.variable(var, line)), body);
                match sorted_by {
                    Some(key) => all.sorted_by(self.expr(key, line)),
                    None => all,
                }
            }
            Query::The { var, .. } => ir::Query::the(self.variable(var, line), body),
        };
        lowered.into()
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
                let value = of.as_ref().map(|e| self.expr(e, line));
                let body = body.iter().map(|l| self.literal(l, line, context)).collect::<Vec<_>>();
                ir::Literal::aggregate(self.variable(var, line), aggregate(*op, value), body)
            }
            Literal::First { var, of, by, body } => {
                let key = self.expr(by, line);
                let body = body.iter().map(|l| self.literal(l, line, context)).collect::<Vec<_>>();
                let candidate = ir::Expr::Variable(self.variable(of, line));
                ir::Literal::argmin(self.variable(var, line), candidate, key, body)
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
        if let (Some(_), Arg::Name(_) | Arg::Int(_) | Arg::Text(_) | Arg::Path(_) | Arg::Expr(_)) =
            (ty.strip_prefix("nix "), arg)
        {
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
            Arg::List(items) => ir::Expr::List(items.iter().map(|a| self.arg(a, ty, line)).collect()),
            Arg::Name(n) => match ty {
                "Bool" => ir::Expr::value(n == "true"),
                "OptionPath" => {
                    ir::Expr::value(ir::Value::OptionPath(n.split('.').map(String::from).collect()))
                }
                _ => {
                    let v = &self.parsed.vocabulary;
                    if v.types.get(ty).is_some_and(Option::is_none) && !v.entities.get(n).is_some_and(|t| t.contains(ty)) {
                        let names = v.entities.iter().filter(|(_, t)| t.contains(ty)).map(|(name, _)| name.as_str());
                        let message = match crate::check::closest(n, names) {
                            Some(near) => format!("`{n}` isn't a declared {ty}; the nearest is `{near}`, and `{n} is a {ty}` declares a new one"),
                            None => format!("`{n}` isn't a declared {ty}; `{n} is a {ty}` declares a new one"),
                        };
                        self.diagnose(line, message);
                    }
                    ir::Expr::value(ir::Value::entity(ty, n.as_str()))
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
            ExprKind::Name(n) if self.params.contains(n) => ir::Expr::var(n.as_str()),
            ExprKind::Name(n) => match self.parsed.vocabulary.entities.get(n) {
                Some(types) if types.len() == 1 => {
                    ir::Expr::value(ir::Value::entity(types.first().unwrap().as_str(), n.as_str()))
                }
                Some(types) => {
                    let types: Vec<&str> = types.iter().map(String::as_str).collect();
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
                    TextPart::Interpolation(e) => self.expr(e, line),
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
            ExprKind::Query(query) => self.query(query, line),
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
                ty: slot.ty.clone(),
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
        if let Some(inner) = ty.strip_prefix("nix ") {
            return self.ty(inner, line).nix();
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
                Some(None) => ir::Type::entity(ty),
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
    declared: Vec<String>,
    line: usize,
    body: Vec<(usize, Literal)>,
    direct: Vec<(String, usize)>,
}

fn ground(value: ir::Expr) -> Option<ir::Value> {
    let mut p = ir::Program::builder();
    let id = p.relation(ir::Relation::new(
        "ground",
        ir::RelationKind::Derived,
        vec![ir::Slot::new("value", ir::Type::Any)],
    ));
    p.rule(ir::Rule::new("ground", ir::Atom::new(id, [value])));
    let model = muove_core::evaluate(&p.build(), &[], muove_core::Budget::default()).ok()?;
    let mut tuples = model.tuples(id);
    tuples.next().and_then(|t| t.first().cloned())
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
