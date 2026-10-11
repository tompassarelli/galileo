use crate::baseline::{definition_key, rule_key};
use crate::derived::shared;
use galileo_core::check::expression_reads;
use galileo_core::{Atom, Claim, Expr, Literal, Program, Query, RelationId, RelationKind, SourceSpan, StateClaim};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    pub at: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefinitionRead {
    pub reader: String,
    pub name: String,
    pub change: &'static str,
    pub before: Vec<Text>,
    pub after: Vec<Text>,
}

impl fmt::Display for DefinitionRead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} reads {}, which this patch {}", self.reader, self.name, self.change)?;
        for (side, texts) in [("before", &self.before), ("after", &self.after)] {
            for text in texts {
                write!(f, "\n  {side}, {}:", text.at)?;
                for line in text.text.lines() {
                    write!(f, "\n    {line}")?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct Reach {
    relations: BTreeSet<RelationId>,
    calls: BTreeSet<String>,
}

impl Reach {
    fn atom(&mut self, program: &Program, atom: &Atom) {
        self.relations.insert(atom.relation);
        for argument in &atom.arguments {
            self.expression(program, argument);
        }
    }
    fn body(&mut self, program: &Program, body: &[Literal]) {
        for literal in body {
            match literal {
                Literal::Positive(atom) | Literal::Negative(atom) => self.atom(program, atom),
                Literal::Compare { left, right, .. } => {
                    self.expression(program, left);
                    self.expression(program, right);
                }
                Literal::Boolean(value) | Literal::Bind { value, .. } | Literal::Member { list: value, .. } => self.expression(program, value),
                Literal::Aggregate { op, body, .. } => {
                    self.body(program, body);
                    if let Some(value) = op.value() {
                        self.expression(program, value);
                    }
                }
                Literal::ArgMin { candidate, key, body, .. } => {
                    self.body(program, body);
                    self.expression(program, candidate);
                    self.expression(program, key);
                }
            }
        }
    }
    fn query(&mut self, program: &Program, query: &Query) {
        self.body(program, &query.body);
        for extra in query.head.iter().chain(&query.sorted_by) {
            self.expression(program, extra);
        }
    }
    fn expression(&mut self, program: &Program, expression: &Expr) {
        self.relations.extend(expression_reads(program, expression));
        match expression {
            Expr::Value(_) | Expr::Variable(_) => {}
            Expr::List(values) => values.iter().for_each(|v| self.expression(program, v)),
            Expr::Record(values) | Expr::Attrs(values) => values.values().for_each(|v| self.expression(program, v)),
            Expr::Field { value, .. } | Expr::Unary { value, .. } | Expr::Lambda { body: value, .. } => self.expression(program, value),
            Expr::Binary { left, right, .. } => {
                self.expression(program, left);
                self.expression(program, right);
            }
            Expr::Call { function, arguments } => {
                arguments.iter().for_each(|v| self.expression(program, v));
                if self.calls.insert(function.clone()) {
                    if let Some(definition) = program.definitions.iter().find(|d| d.name == *function) {
                        self.expression(program, &definition.body);
                    }
                    if let Some(query) = program.queries.iter().find(|q| q.name == *function) {
                        self.query(program, &query.query);
                    }
                }
            }
            Expr::If { condition, then_value, else_value } => {
                for value in [condition, then_value, else_value] {
                    self.expression(program, value);
                }
            }
            Expr::Apply { function, arguments } => {
                self.expression(program, function);
                arguments.iter().for_each(|v| self.expression(program, v));
            }
            Expr::Query(query) => self.query(program, query),
        }
    }
    fn state(&mut self, program: &Program, claim: &StateClaim) {
        match claim {
            StateClaim::Holds(atom) => self.atom(program, atom),
            StateClaim::Some { body, .. } | StateClaim::No { body, .. } => self.body(program, body),
            StateClaim::Every { domain, body, .. } => {
                self.body(program, domain);
                self.body(program, body);
            }
            StateClaim::Before(claim) => self.state(program, claim),
        }
    }
    fn claim(&mut self, program: &Program, claim: &Claim) {
        match claim {
            Claim::State(state) | Claim::Eventually(state) | Claim::Always(state) | Claim::Never(state) => self.state(program, state),
            Claim::Requires { target, condition } => {
                self.atom(program, target);
                self.atom(program, condition);
            }
            Claim::NoSinglePointOfFailure { claim, among } => {
                self.claim(program, claim);
                self.atom(program, among);
            }
            Claim::EveryTracePasses { target, checkpoint, .. } => {
                self.state(program, target);
                self.state(program, checkpoint);
            }
            Claim::EveryEventWhen { event, condition } => {
                self.atom(program, event);
                self.state(program, condition);
            }
        }
    }
    fn close(mut self, program: &Program) -> Self {
        let mut done = BTreeSet::new();
        while let Some(relation) = self.relations.difference(&done).next().copied() {
            done.insert(relation);
            for rule in program.rules.iter().filter(|r| r.head.relation == relation) {
                self.body(program, &rule.body);
                for argument in &rule.head.arguments {
                    self.expression(program, argument);
                }
            }
        }
        self
    }
}

fn readers(program: &Program) -> Vec<(String, Reach)> {
    let mut readers: Vec<(String, Reach)> = program
        .requirements
        .iter()
        .map(|requirement| {
            let mut reach = Reach::default();
            reach.claim(program, &requirement.claim);
            (format!("`{}`", requirement.name), reach.close(program))
        })
        .collect();
    for (index, relation) in program.relations.iter().enumerate().filter(|(_, r)| r.checkpoint) {
        let mut reach = Reach::default();
        reach.relations.insert(RelationId(index));
        readers.push((format!("milestone `{}`", relation.template), reach.close(program)));
    }
    readers
}

fn texts(spans: impl IntoIterator<Item = SourceSpan>, sources: &BTreeMap<String, String>) -> Vec<Text> {
    spans
        .into_iter()
        .map(|span| {
            let lines: Vec<&str> = sources.get(&span.file).map(|s| s.lines().skip(span.line.saturating_sub(1)).collect()).unwrap_or_default();
            let mut block = Vec::new();
            for (index, line) in lines.iter().enumerate() {
                if index > 0 && (line.trim().is_empty() || !line.starts_with(' ')) {
                    break;
                }
                block.push(*line);
            }
            Text { at: format!("{}:{}", span.file, span.line), text: block.join("\n") }
        })
        .collect()
}

enum Target {
    Relation(String),
    Rule(String),
    Definition(String),
}

struct Edited {
    target: Target,
    name: String,
    change: &'static str,
    before: Vec<SourceSpan>,
    after: Vec<SourceSpan>,
}

fn edited(before: &Program, after: &Program) -> Vec<Edited> {
    let names: BTreeSet<&str> = before.relations.iter().chain(&after.relations).map(|r| r.name.as_str()).collect();
    let joint: BTreeMap<&str, usize> = names.into_iter().enumerate().map(|(index, name)| (name, index)).collect();
    let (before_ids, after_ids) = (shared(before, &joint), shared(after, &joint));
    let mut edits = Vec::new();
    let change = |old: bool, new: bool| match (old, new) {
        (true, false) => "removes",
        (false, true) => "adds",
        _ => "replaces",
    };

    let shapes = |program: &Program| -> BTreeMap<String, (String, String, Vec<SourceSpan>)> {
        program
            .relations
            .iter()
            .filter(|r| r.kind == RelationKind::Derived)
            .map(|r| {
                let mut stripped = r.clone();
                stripped.source = None;
                (r.name.clone(), (r.template.clone(), format!("{stripped:?}"), r.source.iter().cloned().collect()))
            })
            .collect()
    };
    let (old, new) = (shapes(before), shapes(after));
    for name in old.keys().chain(new.keys().filter(|k| !old.contains_key(*k))) {
        let (o, n) = (old.get(name), new.get(name));
        if o.map(|s| &s.1) != n.map(|s| &s.1) {
            edits.push(Edited {
                target: Target::Relation(name.clone()),
                name: format!("the derived shape `{}`", n.or(o).map(|s| s.0.as_str()).unwrap_or_default()),
                change: change(o.is_some(), n.is_some()),
                before: o.map(|s| s.2.clone()).unwrap_or_default(),
                after: n.map(|s| s.2.clone()).unwrap_or_default(),
            });
        }
    }

    let rules = |program: &Program, ids: &BTreeMap<RelationId, RelationId>| {
        let mut grouped: BTreeMap<String, (Vec<String>, Vec<SourceSpan>)> = BTreeMap::new();
        for rule in &program.rules {
            let entry = grouped.entry(rule.name.clone()).or_default();
            entry.0.push(rule_key(rule, ids));
            entry.1.extend(rule.source.clone());
        }
        for entry in grouped.values_mut() {
            entry.0.sort();
        }
        grouped
    };
    let (old, new) = (rules(before, &before_ids), rules(after, &after_ids));
    for name in old.keys().chain(new.keys().filter(|k| !old.contains_key(*k))) {
        let (o, n) = (old.get(name), new.get(name));
        if o.map(|r| &r.0) != n.map(|r| &r.0) {
            edits.push(Edited {
                target: Target::Rule(name.clone()),
                name: format!("rule `{name}`"),
                change: change(o.is_some(), n.is_some()),
                before: o.map(|r| r.1.clone()).unwrap_or_default(),
                after: n.map(|r| r.1.clone()).unwrap_or_default(),
            });
        }
    }

    let definitions = |program: &Program, ids: &BTreeMap<RelationId, RelationId>| {
        program.definitions.iter().map(|d| (d.name.clone(), (definition_key(d, ids), d.source.iter().cloned().collect::<Vec<_>>()))).collect::<BTreeMap<_, _>>()
    };
    let (old, new) = (definitions(before, &before_ids), definitions(after, &after_ids));
    for name in old.keys().chain(new.keys().filter(|k| !old.contains_key(*k))) {
        let (o, n) = (old.get(name), new.get(name));
        if o.map(|d| &d.0) != n.map(|d| &d.0) {
            edits.push(Edited {
                target: Target::Definition(name.clone()),
                name: format!("definition `{name}`"),
                change: change(o.is_some(), n.is_some()),
                before: o.map(|d| d.1.clone()).unwrap_or_default(),
                after: n.map(|d| d.1.clone()).unwrap_or_default(),
            });
        }
    }
    edits
}

fn reads_target(program: &Program, reach: &Reach, target: &Target) -> bool {
    match target {
        Target::Relation(name) => program.relation_id(name).is_some_and(|id| reach.relations.contains(&id)),
        Target::Rule(name) => program.rules.iter().any(|r| r.name == *name && reach.relations.contains(&r.head.relation)),
        Target::Definition(name) => reach.calls.contains(name),
    }
}

pub fn definition_reads(
    before: &Program,
    after: &Program,
    before_sources: &BTreeMap<String, String>,
    after_sources: &BTreeMap<String, String>,
) -> Vec<DefinitionRead> {
    let (old_readers, new_readers) = (readers(before), readers(after));
    let mut reads = Vec::new();
    for edit in edited(before, after) {
        let mut names: Vec<&String> = Vec::new();
        for (program, readers) in [(before, &old_readers), (after, &new_readers)] {
            for (reader, reach) in readers {
                if !names.contains(&reader) && reads_target(program, reach, &edit.target) {
                    names.push(reader);
                }
            }
        }
        for reader in names {
            reads.push(DefinitionRead {
                reader: reader.clone(),
                name: edit.name.clone(),
                change: edit.change,
                before: texts(edit.before.clone(), before_sources),
                after: texts(edit.after.clone(), after_sources),
            });
        }
    }
    reads
}

pub(crate) fn claim_reads(program: &Program, claim: &Claim) -> BTreeSet<RelationId> {
    let mut reach = Reach::default();
    reach.claim(program, claim);
    reach.relations
}
