use muove_core::ir::{Atom, Expr, Literal, Program, QueryKind, Rule, Variable};
use muove_syntax::lower::lower;
use muove_syntax::{read_sentences, Error, Sentences};
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
        let identity = identity(&program);
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
    let text = without_spans(&format!("{normal:?}"));
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    Identity(hash)
}

fn without_spans(text: &str) -> String {
    const OPEN: &str = "SourceSpan {";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(OPEN) {
        out.push_str(&rest[..at + "SourceSpan".len()]);
        let mut chars = rest[at + OPEN.len()..].char_indices();
        let mut quoted = false;
        let mut end = rest.len() - at - OPEN.len();
        while let Some((i, c)) = chars.next() {
            match c {
                '\\' if quoted => {
                    chars.next();
                }
                '"' => quoted = !quoted,
                '}' if !quoted => {
                    end = i + 1;
                    break;
                }
                _ => {}
            }
        }
        rest = &rest[at + OPEN.len() + end..];
    }
    out.push_str(rest);
    out
}

#[derive(Default)]
struct Names(BTreeMap<String, String>);

impl Names {
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
    atom.arguments.iter_mut().for_each(|e| expr(e, names));
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
            Literal::Aggregate {
                variable,
                value,
                body,
                ..
            } => {
                names.rename(variable);
                expr(value, names);
                literals(body, names);
            }
            Literal::ArgMin {
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
        Expr::Lambda { body, .. } => expr(body, names),
        Expr::Apply {
            function,
            arguments,
        } => {
            expr(function, names);
            arguments.iter_mut().for_each(|i| expr(i, names));
        }
        Expr::Query(query) => {
            match &mut query.kind {
                QueryKind::All(vars) => vars.iter_mut().for_each(|v| names.rename(v)),
                QueryKind::The(v) => names.rename(v),
            }
            literals(&mut query.body, names);
            if let Some(key) = &mut query.sorted_by {
                expr(key, names);
            }
        }
    }
}
