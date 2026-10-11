use galileo_core::ir::{Action, AggregateOp, Atom, Claim, Expr, Literal, Program, Query, QueryKind, RelationId, Rule, StateClaim, Variable};
use galileo_syntax::lower::lower;
use galileo_syntax::{blocks, read_sentences, Error, Sentences, TokenKind};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Identity(pub u64);

impl fmt::Display for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

#[derive(Clone, Debug)]
pub struct Baseline {
    source: String,
    sentences: Sentences,
    program: Program,
    identity: Identity,
}

impl Baseline {
    pub fn freeze(source: &str) -> Result<Self, Error> {
        let sentences = read_sentences(source)?;
        let program = lower(source, "baseline")?.program;
        let mut exports = sentences.exports.clone();
        for shape in &mut exports.shapes {
            shape.line = 0;
        }
        exports.shapes.sort_by_key(|shape| format!("{shape:?}"));
        let top = blocks(source)?;
        let records = top
            .iter()
            .filter_map(galileo_syntax::record::record_decl)
            .collect::<Result<Vec<_>, _>>()?;
        let private_definitions: Vec<_> = top
            .iter()
            .filter_map(|block| match block.line.tokens.as_slice() {
                [private, define, name, ..]
                    if matches!(&private.kind, TokenKind::Word(word) if word == "private")
                        && matches!(&define.kind, TokenKind::Word(word) if word == "define") =>
                {
                    Some(&name.kind)
                }
                _ => None,
            })
            .collect();
        let identity = hash(&without_spans(&format!(
            "{:?}{exports:?}{records:?}{private_definitions:?}",
            identity(&program)
        )));
        Ok(Self {
            source: source.into(),
            sentences,
            program,
            identity,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn sentences(&self) -> &Sentences {
        &self.sentences
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    pub fn identity(&self) -> Identity {
        self.identity
    }
}

pub fn identity(program: &Program) -> Identity {
    let mut normal = program.clone();
    for rule in &mut normal.rules {
        canonical_variables(rule);
    }
    for requirement in &mut normal.requirements {
        requirement.name.clear();
        requirement.claim = normalized_claim(&requirement.claim, BTreeMap::new()).0;
    }
    for query in &mut normal.queries {
        let mut names = Names(BTreeMap::new(), BTreeMap::new(), true);
        local_query(&mut query.query, &mut names);
    }
    for definition in &mut normal.definitions {
        let mut names = Names::seeded(definition.parameters.iter().map(|slot| slot.name.clone()));
        expr(&mut definition.body, &mut names);
    }
    for view in &mut normal.views {
        let mut names = Names::seeded(view.parameters.iter().map(|slot| slot.name.clone()));
        for (_, value) in &mut view.fields {
            expr(value, &mut names);
        }
    }
    for handler in &mut normal.handlers {
        let mut names = Names(BTreeMap::new(), BTreeMap::new(), true);
        atom(&mut handler.event, &mut names);
        for clause in &mut handler.clauses {
            let mut names = Names(names.0.clone(), BTreeMap::new(), true);
            literals(&mut clause.body, &mut names);
            for action in &mut clause.actions {
                match action {
                    Action::Set(a) | Action::Include(a) | Action::Withdraw(a) | Action::Emit(a) => atom(a, &mut names),
                    Action::Create { variable, fields, .. } => {
                        names.rename(variable);
                        fields.values_mut().for_each(|e| expr(e, &mut names));
                    }
                    Action::Delete(e) => expr(e, &mut names),
                }
            }
            clause.ensures.iter_mut().for_each(|ensure| state(ensure, &mut names));
        }
    }
    let ranks = relation_ranks(&normal);
    let unordered = |items: Vec<String>| {
        let mut items: Vec<_> = items.iter().map(|item| renumbered(&without_spans(item), &ranks)).collect();
        items.sort();
        items
    };
    fn each<T: fmt::Debug>(items: &[T]) -> Vec<String> {
        items.iter().map(|item| format!("{item:?}")).collect()
    }
    let sections = [
        unordered(each(&normal.entity_types)),
        unordered(each(&normal.entities)),
        unordered(each(&normal.relations)),
        unordered(each(&normal.facts)),
        unordered(each(&normal.initially)),
        unordered(each(&normal.rules)),
        unordered(each(&normal.queries)),
        unordered(each(&normal.definitions)),
        unordered(each(&normal.events)),
        unordered(each(&normal.handlers)),
        unordered(each(&normal.effects)),
        unordered(each(&normal.requirements)),
        unordered(each(&normal.views)),
    ];
    hash(&format!("{sections:?}"))
}

fn relation_ranks(program: &Program) -> BTreeMap<usize, usize> {
    let mut order: Vec<usize> = (0..program.relations.len()).collect();
    order.sort_by(|a, b| program.relations[*a].name.cmp(&program.relations[*b].name).then(a.cmp(b)));
    order.into_iter().enumerate().map(|(rank, id)| (id, rank)).collect()
}

fn renumbered(text: &str, ranks: &BTreeMap<usize, usize>) -> String {
    let open = "RelationId(";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(open) {
        out.push_str(&rest[..at + open.len()]);
        rest = &rest[at + open.len()..];
        let digits = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
        match rest[..digits].parse::<usize>().ok().and_then(|id| ranks.get(&id)) {
            Some(rank) => out.push_str(&rank.to_string()),
            None => out.push_str(&rest[..digits]),
        }
        rest = &rest[digits..];
    }
    out.push_str(rest);
    out
}

fn hash(text: &str) -> Identity {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    Identity(hash)
}

fn without_spans(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();
    let mut quoted = false;
    while let Some((at, c)) = chars.next() {
        if !quoted {
            if let Some(open) = ["SourceSpan {", "Span {"]
                .into_iter()
                .find(|open| text[at..].starts_with(*open))
            {
                out.push_str(&open[..open.len() - 2]);
                while chars.peek().is_some_and(|(i, _)| *i < at + open.len()) {
                    chars.next();
                }
                let mut span_quoted = false;
                while let Some((_, c)) = chars.next() {
                    match c {
                        '\\' if span_quoted => {
                            chars.next();
                        }
                        '"' => span_quoted = !span_quoted,
                        '}' if !span_quoted => break,
                        _ => {}
                    }
                }
                continue;
            }
            if text[at..].starts_with("line: ") {
                out.push_str("line: ");
                while chars.peek().is_some_and(|(i, c)| {
                    *i < at + "line: ".len() || c.is_ascii_digit()
                }) {
                    chars.next();
                }
                continue;
            }
        }
        out.push(c);
        match c {
            '\\' if quoted => {
                if let Some((_, escaped)) = chars.next() {
                    out.push(escaped);
                }
            }
            '"' => quoted = !quoted,
            _ => {}
        }
    }
    out
}

#[derive(Default)]
struct Names(BTreeMap<String, String>, BTreeMap<RelationId, RelationId>, bool);

impl Names {
    fn seeded(parameters: impl IntoIterator<Item = String>) -> Self {
        Names(parameters.into_iter().map(|name| (name.clone(), name)).collect(), BTreeMap::new(), true)
    }

    fn rename(&mut self, variable: &mut Variable) {
        let next = format!("v{}", self.0.len());
        variable.0 = self.0.entry(variable.0.clone()).or_insert(next).clone();
    }
}

fn canonical_variables(rule: &mut Rule) {
    let mut names = Names::default();
    atom(&mut rule.head, &mut names);
    literals(&mut rule.body, &mut names);
}

fn atom(atom: &mut Atom, names: &mut Names) {
    if names.2 {
        atom.source = None;
    }
    if let Some(relation) = names.1.get(&atom.relation) {
        atom.relation = *relation;
    }
    atom.arguments.iter_mut().for_each(|e| expr(e, names));
}

fn state(claim: &mut StateClaim, names: &mut Names) {
    match claim {
        StateClaim::Holds(a) => atom(a, names),
        StateClaim::Some { variables, body } | StateClaim::No { variables, body } => {
            variables.iter_mut().for_each(|v| names.rename(v));
            literals(body, names);
        }
        StateClaim::Every { variables, domain, body } => {
            variables.iter_mut().for_each(|v| names.rename(v));
            literals(domain, names);
            literals(body, names);
        }
        StateClaim::Before(inner) | StateClaim::After(inner) => state(inner, names),
    }
}

pub(crate) fn normalized_claim(claim: &Claim, relations: BTreeMap<RelationId, RelationId>) -> (Claim, BTreeMap<String, String>) {
    fn normalize(claim: &mut Claim, names: &mut Names) {
        match claim {
            Claim::State(s) | Claim::Always(s) | Claim::Eventually(s) | Claim::Never(s) => state(s, names),
            Claim::Requires { target, condition } => {
                atom(target, names);
                atom(condition, names);
            }
            Claim::NoSinglePointOfFailure { claim, among } => {
                normalize(claim, names);
                atom(among, names);
            }
            Claim::EveryTracePasses { variables, target, checkpoint } => {
                variables.iter_mut().for_each(|v| names.rename(v));
                state(target, names);
                state(checkpoint, names);
            }
            Claim::EveryEventWhen { event, condition } => {
                atom(event, names);
                state(condition, names);
            }
        }
    }
    let mut claim = claim.clone();
    let mut names = Names(BTreeMap::new(), relations, true);
    normalize(&mut claim, &mut names);
    (claim, names.0)
}

fn literals(body: &mut [Literal], names: &mut Names) {
    for literal in body {
        match literal {
            Literal::Positive(a) | Literal::Negative(a) => atom(a, names),
            Literal::Compare { left, right, .. } => {
                expr(left, names);
                expr(right, names);
            }
            Literal::Boolean(e) => expr(e, names),
            Literal::Bind { variable, value } => {
                names.rename(variable);
                expr(value, names);
            }
            Literal::Member { variable, list } => {
                names.rename(variable);
                expr(list, names);
            }
            Literal::Aggregate { variable, op, body } => {
                names.rename(variable);
                if let AggregateOp::Sum(value) | AggregateOp::Min(value) | AggregateOp::Max(value) = op {
                    expr(value, names);
                }
                literals(body, names);
            }
            Literal::ArgMin {
                direction: _,
                variable,
                candidate,
                key,
                body,
            } => {
                names.rename(variable);
                expr(candidate, names);
                expr(key, names);
                literals(body, names);
            }
        }
    }
}

fn expr(e: &mut Expr, names: &mut Names) {
    match e {
        Expr::Value(_) => {}
        Expr::Variable(v) => names.rename(v),
        Expr::List(items) => items.iter_mut().for_each(|i| expr(i, names)),
        Expr::Record(fields) | Expr::Attrs(fields) => {
            fields.values_mut().for_each(|i| expr(i, names))
        }
        Expr::Field { value, .. } | Expr::Unary { value, .. } => expr(value, names),
        Expr::Binary { left, right, .. } => {
            expr(left, names);
            expr(right, names);
        }
        Expr::Call { arguments, .. } => arguments.iter_mut().for_each(|i| expr(i, names)),
        Expr::If {
            condition,
            then_value,
            else_value,
        } => {
            expr(condition, names);
            expr(then_value, names);
            expr(else_value, names);
        }
        Expr::Lambda { parameters, body } => {
            let outer = names.2.then(|| names.0.clone());
            if names.2 {
                let start = names.0.len();
                for (index, parameter) in parameters.iter_mut().enumerate() {
                    let renamed = format!("v{}", start + index);
                    names.0.insert(parameter.name.clone(), renamed.clone());
                    parameter.name = renamed;
                }
            }
            expr(body, names);
            if let Some(outer) = outer { names.0 = outer; }
        }
        Expr::Apply {
            function,
            arguments,
        } => {
            expr(function, names);
            arguments.iter_mut().for_each(|i| expr(i, names));
        }
        Expr::Query(query) => local_query(query, names),
    }
}

fn local_query(query: &mut Query, names: &mut Names) {
    query.source = None;
    match &mut query.kind {
        QueryKind::All(vars) => vars.iter_mut().for_each(|v| names.rename(v)),
        QueryKind::The(v) => names.rename(v),
    }
    literals(&mut query.body, names);
    if let Some(head) = &mut query.head { expr(head, names); }
    if let Some(key) = &mut query.sorted_by {
        expr(key, names);
    }
}

pub(crate) fn rule_key(rule: &Rule, relations: &BTreeMap<RelationId, RelationId>) -> String {
    let mut rule = rule.clone();
    rule.source = None;
    rule.body_sources.clear();
    let mut names = Names(BTreeMap::new(), relations.clone(), true);
    atom(&mut rule.head, &mut names);
    literals(&mut rule.body, &mut names);
    without_spans(&format!("{rule:?}"))
}

pub(crate) fn definition_key(definition: &galileo_core::ir::Definition, relations: &BTreeMap<RelationId, RelationId>) -> String {
    let mut definition = definition.clone();
    definition.source = None;
    let mut names = Names(BTreeMap::new(), relations.clone(), true);
    expr(&mut definition.body, &mut names);
    without_spans(&format!("{definition:?}"))
}
