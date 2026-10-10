use crate::expr::{parse_expr, BinOp, Expr, ExprKind, Query, TextPart};
use crate::record::Segment;
use crate::{blocks, lex, read_sentences, Arg, Error, Kind, Line, Literal, Reading, Sentences};
use muove_core as ir;
use std::collections::BTreeMap;

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
    let parsed = read_sentences(source)?;
    let lines = lex(source)?;
    let heads = blocks(source)?
        .into_iter()
        .filter(|b| matches!(&b.line.tokens[0].kind, crate::TokenKind::Word(w) if w == "rule"))
        .filter_map(|b| Some((b.line.no, b.children.first()?.line.no)))
        .collect();
    let mut lowering = Lowering {
        parsed: &parsed,
        lines: lines.iter().map(|l| (l.no, l)).collect(),
        file,
        readings: Vec::new(),
        diagnostics: parsed.warnings.clone(),
    };
    let program = lowering.program(&heads);
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
    file: &'a str,
    readings: Vec<SentenceReading>,
    diagnostics: Vec<Error>,
}

impl Lowering<'_> {
    fn program(&mut self, heads: &BTreeMap<usize, usize>) -> ir::Program {
        let v = &self.parsed.vocabulary;
        let mut p = ir::Program::builder();
        for (name, union) in &v.types {
            if union.is_none() {
                p.entity_type(name.clone());
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
            p.relation(relation);
            self.read(shape.line, Role::Shape, index, None);
        }
        for fact in &self.parsed.facts {
            let shape = &v.shapes[fact.shape];
            let mut values = Vec::new();
            for (slot, arg) in shape.slots.iter().zip(&fact.args) {
                match self.arg(arg, &slot.ty, fact.line) {
                    ir::Expr::Value(value) => values.push(value),
                    _ => {
                        self.diagnose(fact.line, format!("`{arg}` isn't a value a fact can state"));
                        values.push(ir::Value::Null);
                    }
                }
            }
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
            for (line, literal) in &rule.body {
                body.push(self.literal(literal, *line, Some(&rule.name)));
            }
            p.rule(
                ir::Rule::new(rule.name.clone(), head)
                    .when(body)
                    .at(self.span(rule.line)),
            );
        }
        p.build()
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
        let body: Vec<ir::Literal> = body.iter().map(|l| self.literal(l, line, None)).collect();
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

    fn literal(&mut self, literal: &Literal, line: usize, rule: Option<&str>) -> ir::Literal {
        let role = |negated| match rule {
            Some(rule) => Role::Condition {
                rule: rule.into(),
                negated,
            },
            None => Role::Query,
        };
        match literal {
            Literal::Sentence(reading) => {
                self.read(line, role(false), reading.shape, Some(&reading.args));
                self.atom(reading, line).positive()
            }
            Literal::Not(reading) => {
                self.read(line, role(true), reading.shape, Some(&reading.args));
                self.atom(reading, line).negative()
            }
            Literal::Compare(op, left, right) => {
                ir::Literal::compare(compare(*op), self.expr(left, line), self.expr(right, line))
            }
            Literal::Holds(e) => ir::Literal::Boolean(self.expr(e, line)),
            Literal::Bind(var, e) => ir::Literal::bind(var.as_str(), self.expr(e, line)),
            Literal::In(var, e) => ir::Literal::member(var.as_str(), self.expr(e, line)),
            Literal::IsA(var, ty) => {
                self.diagnose(
                    line,
                    format!("`?{var} is a {ty}` has no Core IR form yet (DECISION 2.11)"),
                );
                ir::Literal::Boolean(ir::Expr::value(false))
            }
            Literal::Aggregate { var, op, of, body } => {
                let value = match of {
                    Some(e) => self.expr(e, line),
                    None => ir::Expr::Value(ir::Value::Null),
                };
                let body = body.iter().map(|l| self.literal(l, line, rule)).collect::<Vec<_>>();
                ir::Literal::aggregate(var.as_str(), aggregate(*op), value, body)
            }
            Literal::First { var, of, by, body } => {
                let key = self.expr(by, line);
                let body = body.iter().map(|l| self.literal(l, line, rule)).collect::<Vec<_>>();
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
        ir::SourceSpan {
            file: self.file.into(),
            start: tokens.first().map_or(0, |t| t.span.start),
            end: tokens.last().map_or(0, |t| t.span.end),
            line,
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
