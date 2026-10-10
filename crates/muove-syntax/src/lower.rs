use crate::expr::{parse_expr, BinOp, Expr, ExprKind, Query, TextPart};
use crate::record::Segment;
use crate::{
    blocks, lex, read_sentences, Action, Arg, Block, Claim, Error, Kind, Line, Literal, Quantifier,
    Reading, Sentences, Token, TokenKind,
};
use muove_core as ir;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct Lowered {
    pub program: ir::Program,
    pub readings: Vec<SentenceReading>,
    pub diagnostics: Vec<Error>,
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
    pub relation: ir::RelationId,
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
    lower_files(&[(file, source)])
}

/// Lowers several files read as one module, keeping each sentence's own file and line.
pub fn lower_files(files: &[(&str, &str)]) -> Result<Lowered, Error> {
    let mut source = String::new();
    let mut starts = Vec::new();
    for (file, text) in files {
        starts.push((source.matches('\n').count() + 1, file.to_string(), source.len()));
        source.push_str(text);
        if !text.ends_with('\n') {
            source.push('\n');
        }
    }
    let local = |error: Error| locate(&starts, error);
    let mut lowered = lower_joined(&source, &starts).map_err(local)?;
    lowered.diagnostics = lowered.diagnostics.into_iter().map(local).collect();
    for reading in &mut lowered.readings {
        reading.line = place(&starts, reading.line).1;
    }
    Ok(lowered)
}

fn place(starts: &[(usize, String, usize)], line: usize) -> (&str, usize, usize) {
    match starts.iter().rev().find(|(start, _, _)| *start <= line) {
        Some((start, file, offset)) => (file, line - start + 1, *offset),
        None => ("", line, 0),
    }
}

fn locate(starts: &[(usize, String, usize)], mut error: Error) -> Error {
    if error.line > 0 && starts.len() > 1 {
        let (file, line, _) = place(starts, error.line);
        error.message = format!("{file}: {}", error.message);
        error.line = line;
    }
    error
}

fn lower_joined(source: &str, starts: &[(usize, String, usize)]) -> Result<Lowered, Error> {
    let parsed = read_sentences(source)?;
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
    let heads = top
        .iter()
        .filter(|b| word(&b.line.tokens, 0) == "rule")
        .filter_map(|b| Some((b.line.no, b.children.first()?.line.no)))
        .collect();
    let mut lowering = Lowering {
        parsed: &parsed,
        lines: lines.iter().map(|l| (l.no, l)).collect(),
        starts,
        readings: Vec::new(),
        diagnostics: parsed.warnings.clone(),
        params: BTreeSet::new(),
    };
    let program = lowering.program(&heads, &top);
    let mut diagnostics = lowering.diagnostics;
    if let Err(errors) = ir::check(&program) {
        diagnostics.extend(errors.into_iter().map(|e| match &e.source {
            Some(span) => lowering
                .lines
                .get(&span.line)
                .map(|line| Error::at(line, e.to_string()))
                .unwrap_or_else(|| Error::bare(e.to_string())),
            None => Error::bare(e.to_string()),
        }));
    }
    Ok(Lowered {
        program,
        readings: lowering.readings,
        diagnostics,
    })
}

struct Lowering<'a> {
    parsed: &'a Sentences,
    lines: BTreeMap<usize, &'a Line>,
    starts: &'a [(usize, String, usize)],
    readings: Vec<SentenceReading>,
    diagnostics: Vec<Error>,
    params: BTreeSet<String>,
}

impl Lowering<'_> {
    fn program(&mut self, heads: &BTreeMap<usize, usize>, top: &[Block]) -> ir::Program {
        let v = &self.parsed.vocabulary;
        let mut p = ir::Program::builder();
        for (name, union) in &v.types {
            if union.is_some() {
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
        for (name, ty) in &v.entities {
            p.entity(ir::Entity::named(ty.clone(), name.clone()));
        }
        for (index, shape) in v.shapes.iter().enumerate() {
            let slots = shape
                .slots
                .iter()
                .map(|s| ir::Slot::new(s.name.clone(), self.ty(&s.ty, shape.line)))
                .collect::<Vec<_>>();
            let mut relation = ir::Relation::new(shape.name.clone(), kind(shape.kind), slots)
                .template(v.shape_text(index))
                .at(self.span(shape.line));
            if shape.checkpoint {
                relation = relation.checkpoint();
            }
            if let Some(mode) = shape.modes.first() {
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
            if shape.modes.len() > 1 {
                self.diagnose(shape.line, "Core IR holds one mode per relation; the rest aren't lowered");
            }
            let id = p.relation(relation);
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
                    if let Some(receipt) = &shape.settled_by {
                        match v
                            .shapes
                            .iter()
                            .position(|s| &s.name == receipt && s.kind == Kind::Event)
                        {
                            Some(event) => effect = effect.settled_by(ir::RelationId(event)),
                            None => self.diagnose(
                                shape.line,
                                format!("`settled by {receipt}` names no event shape"),
                            ),
                        }
                    }
                    p.effect(effect);
                }
                _ => {}
            }
        }
        for fact in &self.parsed.facts {
            let shape = &v.shapes[fact.shape];
            let values = shape
                .slots
                .iter()
                .zip(&fact.args)
                .map(|(slot, arg)| self.value(arg, &slot.ty, fact.line))
                .collect::<Vec<_>>();
            let fact_ir = ir::Fact::new(ir::RelationId(fact.shape), values).at(self.span(fact.line));
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
            let mut body = Vec::new();
            let context = Context::Rule(rule.name.clone());
            for (line, literal) in &rule.body {
                body.push(self.literal(literal, *line, &context));
            }
            p.rule(
                ir::Rule::new(rule.name.clone(), head)
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
            let (clause, event) = self.handler(handler);
            match handlers.iter_mut().find(|h| h.event.relation == event.relation) {
                Some(earlier) => {
                    if earlier.event.arguments != event.arguments {
                        self.diagnose(
                            handler.line,
                            "clauses of one event name its arguments alike, as its first `on` line does",
                        );
                    }
                    earlier.clauses.push(clause);
                    earlier.may_skip |= handler.may_skip;
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
        p.build()
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
        const FORM: &str = "a definition is `define name(param: Type, ...): Type` with its body indented beneath";
        let line = &block.line;
        if block.children.is_empty() {
            return None;
        }
        let tokens = &line.tokens;
        let open = tokens.iter().position(|t| is_symbol(t, "("));
        let close = tokens.iter().position(|t| is_symbol(t, ")"));
        let (Some(open), Some(close)) = (open, close) else {
            self.diagnose(line.no, FORM);
            return None;
        };
        if open != 2 || close < open || !tokens.get(close + 1).is_some_and(|t| is_symbol(t, ":")) {
            self.diagnose(line.no, FORM);
            return None;
        }
        let parameters = self.parameters(&tokens[open + 1..close], line.no);
        let result = self.type_of(&tokens[close + 2..], line.no);
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
            name: word(tokens, 1).into(),
            parameters,
            result,
            body,
            effect: ir::EffectLevel::Total,
        })
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
        self.params = parameters.iter().map(|s| s.name.clone()).collect();
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
        self.params.clear();
        Some(ir::View {
            name: word(tokens, 1).into(),
            parameters,
            fields,
            source: Some(self.span(line.no)),
        })
    }

    fn handler(&mut self, handler: &crate::Handler) -> (ir::HandlerClause, ir::Atom) {
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
                Action::Create { var, ty } => ir::Action::Create {
                    variable: ir::Variable::new(var.as_str()),
                    entity_type: ty.clone(),
                    fields: BTreeMap::new(),
                },
                Action::Delete(var) => ir::Action::Delete(ir::Expr::var(var.as_str())),
            };
            clause.actions.push(lowered);
        }
        for (line, claim) in &handler.ensures {
            let lowered = self.state_claim(claim, *line);
            clause.ensures.push(lowered);
        }
        (clause, event)
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
                let variables = vars.iter().map(ir::Variable::new).collect();
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
            _ => {
                self.diagnose(line, format!("`{arg}` isn't a value a fact can state"));
                ir::Value::Null
            }
        }
    }

    /// Lowers a parsed query, such as a `nix module`'s `for ?m in all ?m where …`.
    fn query(&mut self, query: &Query, line: usize) -> ir::Expr {
        let Some(&source) = self.lines.get(&line) else {
            self.diagnose(line, "this query's line is missing");
            return ir::Expr::Value(ir::Value::Null);
        };
        let body = match self.parsed.vocabulary.read_query(source, query) {
            Ok(body) => body,
            Err(error) => {
                self.diagnostics.push(error);
                return ir::Expr::Value(ir::Value::Null);
            }
        };
        let body: Vec<ir::Literal> = body.iter().map(|l| self.literal(l, line, &Context::Query)).collect();
        let lowered = match query {
            Query::All {
                vars, sorted_by, ..
            } => {
                let all = ir::Query::all(vars.iter().map(ir::Variable::new), body);
                match sorted_by {
                    Some(key) => all.sorted_by(self.expr(key, line)),
                    None => all,
                }
            }
            Query::The { var, .. } => ir::Query::the(var.as_str(), body),
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
            Literal::Bind(var, e) => ir::Literal::bind(var.as_str(), self.expr(e, line)),
            Literal::In(var, e) => ir::Literal::member(var.as_str(), self.expr(e, line)),
            Literal::IsA(var, ty) => ir::Literal::member(
                var.as_str(),
                ir::Expr::call("entities", [ir::Expr::value(ty.as_str())]),
            ),
            Literal::Aggregate { var, op, of, body } => {
                let value = match of {
                    Some(e) => self.expr(e, line),
                    None => ir::Expr::Value(ir::Value::Null),
                };
                let body = body.iter().map(|l| self.literal(l, line, context)).collect::<Vec<_>>();
                ir::Literal::aggregate(var.as_str(), aggregate(*op), value, body)
            }
            Literal::First { var, of, by, body } => {
                let key = self.expr(by, line);
                let body = body.iter().map(|l| self.literal(l, line, context)).collect::<Vec<_>>();
                ir::Literal::argmin(var.as_str(), ir::Expr::var(of.as_str()), key, body)
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
        ir::Atom::new(ir::RelationId(reading.shape), arguments).at(self.span(line))
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
            Arg::Var(name) => ir::Expr::var(name.as_str()),
            Arg::Int(n) => ir::Expr::value(*n),
            Arg::Text(t) => ir::Expr::value(t.as_str()),
            Arg::Path(p) => ir::Expr::value(ir::Value::Path(p.clone())),
            Arg::List(items) => ir::Expr::List(items.iter().map(|a| self.arg(a, ty, line)).collect()),
            Arg::Name(n) if self.params.contains(n) => ir::Expr::var(n.as_str()),
            Arg::Name(n) => match ty {
                "Bool" => ir::Expr::value(n == "true"),
                "OptionPath" => {
                    ir::Expr::value(ir::Value::OptionPath(n.split('.').map(String::from).collect()))
                }
                _ => {
                    let declared = self.parsed.vocabulary.entities.get(n);
                    ir::Expr::value(ir::Value::entity(declared.map_or(ty, String::as_str), n.as_str()))
                }
            },
            Arg::Expr(text) => match lex(text).map(|mut l| l.remove(0)) {
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
            },
        }
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
            ExprKind::Var(name) => ir::Expr::var(name.as_str()),
            ExprKind::Name(n) if self.params.contains(n) => ir::Expr::var(n.as_str()),
            ExprKind::Name(n) => match self.parsed.vocabulary.entities.get(n) {
                Some(ty) => ir::Expr::value(ir::Value::entity(ty.as_str(), n.as_str())),
                None => {
                    self.diagnose(line, format!("`{n}` isn't a declared entity"));
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
                    ExprKind::Name(name) => ir::Expr::call(name.as_str(), arguments),
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
            relation: ir::RelationId(shape),
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

fn aggregate(op: crate::Aggregate) -> ir::AggregateOp {
    match op {
        crate::Aggregate::Count => ir::AggregateOp::Count,
        crate::Aggregate::Sum => ir::AggregateOp::Sum,
        crate::Aggregate::Min => ir::AggregateOp::Min,
        crate::Aggregate::Max => ir::AggregateOp::Max,
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
