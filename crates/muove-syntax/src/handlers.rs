use crate::expr::{parse_expr, Expr, ExprKind, Query};
use crate::record::Segment;
use crate::rules::{symbol, top_level, variable, word, Literal};
use crate::{units, Block, Error, Kind, Line, Reading, Token, TokenKind, Vocabulary};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Set(Reading),
    Include(Reading),
    Withdraw(Reading),
    Create {
        var: String,
        ty: String,
        fields: Vec<(String, Expr)>,
    },
    Delete { var: String, ty: String },
    Emit(Reading),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handler {
    pub event: Reading,
    pub when: Vec<(usize, Literal)>,
    pub actions: Vec<(usize, Action)>,
    pub ensures: Vec<(usize, Claim)>,
    pub may_skip: bool,
    pub line: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quantifier {
    Some,
    Every,
    No,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Claim {
    State(Vec<Literal>),
    Quantified {
        quantifier: Quantifier,
        vars: Vec<String>,
        range: Vec<Literal>,
        body: Vec<Literal>,
    },
    Eventually(Vec<Literal>),
    Always(Vec<Literal>),
    Never(Vec<Literal>),
    Requires(Vec<Literal>, Vec<Literal>),
    NoSinglePointOfFailure(Box<Claim>, Vec<Literal>),
    TracePasses(Vec<Literal>, Vec<Literal>),
    HappensWhen(Reading, Vec<Literal>),
    Before(Vec<Literal>),
}

impl Claim {
    pub fn is_state(&self) -> bool {
        matches!(self, Claim::State(_) | Claim::Quantified { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Requirement {
    pub invariant: bool,
    pub claim: Claim,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Requirements {
    pub name: Option<String>,
    pub requirements: Vec<Requirement>,
    pub line: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestionKind {
    Sentence,
    Claim,
    Query,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Question {
    Sentence(Reading),
    Claim(Claim),
    Query(Query),
}

const STATE_CLAIM: &str =
    "takes a state claim: a sentence, or a `some`, `no` or `every` claim, checked on one state";

pub(crate) fn read_handler(v: &Vocabulary, block: &Block) -> Result<Handler, Error> {
    let line = &block.line;
    let mut vars = BTreeMap::new();
    let header = units(line, &line.tokens[1..])?;
    if header.is_empty() {
        return Err(Error::at(
            line,
            "`on` is followed by an event sentence, as in `on ?hero moves through ?door`",
        ));
    }
    let event = v.read(line, &header, Some(&mut vars))?;
    let shape = &v.shapes[event.shape];
    if shape.kind != Kind::Event {
        return Err(Error::at(
            line,
            format!(
                "`on` takes an event sentence, but `{}` is {:?}",
                shape.name, shape.kind
            ),
        ));
    }
    let mut handler = Handler {
        event,
        when: Vec::new(),
        actions: Vec::new(),
        ensures: Vec::new(),
        may_skip: false,
        line: line.no,
    };
    let mut bound: BTreeSet<String> = vars.keys().cloned().collect();
    for (index, child) in block.children.iter().enumerate() {
        let l = &child.line;
        let first = l.tokens.first().map(word).unwrap_or("");
        if handler.may_skip {
            return Err(Error::at(l, "`may skip` is the handler's last line"));
        }
        let nested = |what: &str| match child.children.first() {
            Some(deeper) => Err(Error::at(&deeper.line, format!("{what} don't nest"))),
            None => Ok(()),
        };
        match (first, l.tokens.len()) {
            ("when", 1) => {
                if index != 0 {
                    return Err(Error::at(l, "`when` comes first, before `then`"));
                }
                if child.children.is_empty() {
                    return Err(Error::at(l, "`when` needs its conditions indented beneath"));
                }
                for condition in &child.children {
                    if let Some(deeper) = condition.children.first() {
                        return Err(Error::at(&deeper.line, "conditions don't nest"));
                    }
                    let literal = v.literal(&condition.line, &condition.line.tokens, &mut vars)?;
                    handler.when.push((condition.line.no, literal));
                }
                bound.extend(vars.keys().cloned());
            }
            ("ensures", 1) => {
                if !handler.ensures.is_empty() {
                    return Err(Error::at(l, "a handler has one `ensures`"));
                }
                if child.children.is_empty() {
                    return Err(Error::at(l, "`ensures` needs its claims indented beneath"));
                }
                for claim_block in &child.children {
                    if let Some(deeper) = claim_block.children.first() {
                        return Err(Error::at(&deeper.line, "claims don't nest"));
                    }
                    let cl = &claim_block.line;
                    let claim = v.claim(cl, &cl.tokens, &mut vars.clone())?;
                    if !claim.is_state() && !matches!(claim, Claim::Before(_)) {
                        return Err(Error::at(cl, format!("`ensures` {STATE_CLAIM}")));
                    }
                    handler.ensures.push((cl.no, claim));
                }
            }
            ("may", 2) if l.tokens.get(1).map(word) == Some("skip") => {
                nested("handler lines")?;
                handler.may_skip = true;
            }
            ("then", 1) => {
                if !handler.actions.is_empty() {
                    return Err(Error::at(l, "a handler has one `then`"));
                }
                if !handler.ensures.is_empty() {
                    return Err(Error::at(l, "`then` comes before `ensures`"));
                }
                if child.children.is_empty() {
                    return Err(Error::at(l, "`then` needs what holds afterwards indented beneath"));
                }
                for post in &child.children {
                    if let Some(deeper) = post.children.first() {
                        return Err(Error::at(&deeper.line, "`then` lines don't nest"));
                    }
                    let action = v.post(&post.line, &mut vars, &mut bound)?;
                    handler.actions.push((post.line.no, action));
                }
            }
            _ => return Err(old_verb(l, &vars).unwrap_or_else(|| Error::at(l, HANDLER_LINE))),
        }
    }
    if handler.actions.is_empty() && !handler.may_skip {
        return Err(Error::at(
            line,
            "a handler states what holds afterwards in a `then` block, as in `then` with `?hero is in ?there` beneath",
        ));
    }
    Ok(handler)
}

pub(crate) fn read_requirements(v: &Vocabulary, block: &Block) -> Result<Requirements, Error> {
    let line = &block.line;
    let name = match &line.tokens[1..] {
        [] => None,
        [f, rest @ ..] if word(f) == "for" && !rest.is_empty() => {
            Some(rest.iter().map(word).collect::<Vec<_>>().join(" "))
        }
        _ => {
            return Err(Error::at(
                line,
                "a requirements block is `requirements` or `requirements for <words>`",
            ))
        }
    };
    if block.children.is_empty() {
        return Err(Error::at(line, "list the requirements indented beneath"));
    }
    let mut requirements = Vec::new();
    for child in &block.children {
        let l = &child.line;
        if let Some(deeper) = child.children.first() {
            return Err(Error::at(&deeper.line, "a requirement is one line"));
        }
        let invariant = l.tokens.first().map(word) == Some("invariant");
        let tokens = &l.tokens[usize::from(invariant)..];
        let claim = v.claim(l, tokens, &mut BTreeMap::new())?;
        if matches!(claim, Claim::Before(_)) {
            return Err(Error::at(
                l,
                "`before` reads a step's pre-state, so it belongs in a handler's `ensures` or a patch contract",
            ));
        }
        if invariant && !claim.is_state() {
            return Err(Error::at(
                l,
                format!(
                    "an `invariant` {STATE_CLAIM}. A temporal claim depends on which steps are admitted; put an event's precondition in its handler's `when`"
                ),
            ));
        }
        requirements.push(Requirement {
            invariant,
            claim,
            line: l.no,
        });
    }
    Ok(Requirements {
        name,
        requirements,
        line: line.no,
    })
}

pub(crate) fn read_question(
    v: &Vocabulary,
    line: &Line,
    kind: QuestionKind,
) -> Result<Question, Error> {
    let tokens = &line.tokens;
    Ok(match kind {
        QuestionKind::Sentence => Question::Sentence(v.read(line, &units(line, tokens)?, None)?),
        QuestionKind::Claim => {
            let claim = v.claim(line, tokens, &mut BTreeMap::new())?;
            if matches!(claim, Claim::Before(_)) {
                return Err(Error::at(
                    line,
                    "`before` reads a step's pre-state, so it belongs in a handler's `ensures` or a patch contract",
                ));
            }
            Question::Claim(claim)
        }
        QuestionKind::Query => {
            let expr = parse_expr(tokens, line)?;
            let ExprKind::Query(query) = expr.kind else {
                return Err(Error::at(
                    line,
                    "expected a query: `all ?x where …` or `the ?x where …`",
                ));
            };
            v.read_query(line, &query)?;
            Question::Query(query)
        }
    })
}

impl Vocabulary {
    fn post(
        &self,
        line: &Line,
        vars: &mut BTreeMap<String, String>,
        bound: &mut BTreeSet<String>,
    ) -> Result<Action, Error> {
        if let Some(error) = old_verb(line, vars) {
            return Err(error);
        }
        let tokens = line.tokens.as_slice();
        let is = |t: &Token, w: &str| matches!(&t.kind, TokenKind::Word(x) if x == w);
        if let [var, i, a, n, ty, record @ ..] = tokens {
            if variable(var).is_some() && is(i, "is") && is(a, "a") && is(n, "new") {
                let var = variable(var).unwrap();
                let ty = word(ty);
                if !self.types.get(ty).is_some_and(Option::is_none) {
                    return Err(Error::at(line, format!("`is a new` makes an entity, but `{ty}` isn't an entity type")));
                }
                let fields = if record.is_empty() {
                    Vec::new()
                } else {
                    new_fields(line, record)?
                };
                vars.insert(var.clone(), ty.into());
                bound.insert(var.clone());
                return Ok(Action::Create { var, ty: ty.into(), fields });
            }
        }
        if let [not, var, i, a, ty] = tokens {
            if is(not, "not") && variable(var).is_some() && is(i, "is") && is(a, "a") {
                let var = variable(var).unwrap();
                if !self.types.get(word(ty)).is_some_and(Option::is_none) {
                    return Err(Error::at(line, format!("`not ?{var} is a T` deletes an entity, but `{}` isn't an entity type", word(ty))));
                }
                if !bound.contains(&var) {
                    return Err(unbound(line, &var));
                }
                let ty = vars.get(&var).cloned().unwrap_or_default();
                return Ok(Action::Delete { var, ty });
            }
        }
        let withdraw = tokens.first().is_some_and(|t| is(t, "not"));
        let rest = &tokens[usize::from(withdraw)..];
        if rest.is_empty() {
            return Err(Error::at(line, "`not` is followed by the state sentence it withdraws"));
        }
        let reading = self.read(line, &units(line, rest)?, Some(vars))?;
        for arg in &reading.args {
            if let crate::Arg::Var(name) = arg {
                if name != "_" && !bound.contains(name) {
                    return Err(unbound(line, name));
                }
            }
        }
        let shape = &self.shapes[reading.shape];
        Ok(match (shape.kind, withdraw) {
            (Kind::State, true) => Action::Withdraw(reading),
            (Kind::State, false) if shape.modes.len() == 1 => Action::Set(reading),
            (Kind::State, false) => Action::Include(reading),
            (Kind::Effect, false) => Action::Emit(reading),
            (kind, _) => {
                let what = if withdraw { "a state sentence" } else { "a state or effect sentence" };
                return Err(Error::at(
                    line,
                    format!("a `then` line {}takes {what}, but `{}` is {kind:?}", if withdraw { "`not` " } else { "" }, shape.name),
                ));
            }
        })
    }

    pub(crate) fn claim(
        &self,
        line: &Line,
        tokens: &[Token],
        vars: &mut BTreeMap<String, String>,
    ) -> Result<Claim, Error> {
        if tokens.is_empty() {
            return Err(Error::at(line, "expected a claim"));
        }
        const SPOF: [&str; 7] = ["has", "no", "single", "point", "of", "failure", "among"];
        if let Some(at) = phrase(tokens, &SPOF) {
            let left = self.claim(line, &tokens[..at], vars)?;
            let among = self.pattern(line, &tokens[at + SPOF.len()..], vars, "among")?;
            return Ok(Claim::NoSinglePointOfFailure(Box::new(left), among));
        }
        let rest = &tokens[1..];
        match word(&tokens[0]) {
            "eventually" => {
                return Ok(Claim::Eventually(self.pattern(
                    line,
                    rest,
                    vars,
                    "eventually",
                )?))
            }
            "always" => return Ok(Claim::Always(self.pattern(line, rest, vars, "always")?)),
            "never" => return Ok(Claim::Never(self.pattern(line, rest, vars, "never")?)),
            "before" => return Ok(Claim::Before(self.pattern(line, rest, vars, "before")?)),
            "every" if rest.first().map(word) == Some("trace") => {
                let expected = "write `every trace to S passes C`";
                if rest.get(1).map(word) != Some("to") {
                    return Err(Error::at(line, expected));
                }
                let Some(at) = phrase(&rest[2..], &["passes"]) else {
                    return Err(Error::at(line, expected));
                };
                let to = self.pattern(line, &rest[2..2 + at], vars, "every trace to")?;
                let passes = self.pattern(line, &rest[3 + at..], vars, "passes")?;
                return Ok(Claim::TracePasses(to, passes));
            }
            "every" if phrase(tokens, &["happens", "when"]).is_some() => {
                let at = phrase(tokens, &["happens", "when"]).unwrap();
                let event_units = units(line, &tokens[1..at])?;
                let event = self.read(line, &event_units, Some(vars))?;
                let shape = &self.shapes[event.shape];
                if shape.kind != Kind::Event {
                    return Err(Error::at(
                        line,
                        format!(
                            "`every … happens when` takes an event sentence, but `{}` is {:?}",
                            shape.name, shape.kind
                        ),
                    ));
                }
                let when = self.pattern(line, &tokens[at + 2..], vars, "happens when")?;
                return Ok(Claim::HappensWhen(event, when));
            }
            q @ ("some" | "every" | "no") if rest.first().and_then(variable).is_some() => {
                let quantifier = match q {
                    "some" => Quantifier::Some,
                    "every" => Quantifier::Every,
                    _ => Quantifier::No,
                };
                return self.quantified(line, quantifier, rest, vars);
            }
            _ => {}
        }
        if let Some(at) = phrase(tokens, &["requires"]) {
            let s = self.pattern(line, &tokens[..at], vars, "requires")?;
            let c = self.pattern(line, &tokens[at + 1..], vars, "requires")?;
            return Ok(Claim::Requires(s, c));
        }
        if let Some(var) = tokens
            .iter()
            .filter_map(variable)
            .find(|v| v != "_" && !vars.contains_key(v))
        {
            return Err(Error::at(
                line,
                format!(
                    "`?{var}` isn't bound: say which with `some ?{var}: …`, `every ?{var}: …` or `no ?{var}: …`"
                ),
            ));
        }
        Ok(Claim::State(self.condition(line, tokens, vars)?))
    }

    fn quantified(
        &self,
        line: &Line,
        quantifier: Quantifier,
        tokens: &[Token],
        vars: &mut BTreeMap<String, String>,
    ) -> Result<Claim, Error> {
        let outer: BTreeSet<String> = vars.keys().cloned().collect();
        let expected =
            "a quantified claim is `every ?x, ?y where <condition>: <claim>` (`where` is optional)";
        let mut names = Vec::new();
        let mut at = 0;
        while let Some(name) = tokens.get(at).and_then(variable) {
            names.push(name);
            at += 1;
            if tokens.get(at).is_some_and(|t| symbol(t, ",")) {
                at += 1;
            } else {
                break;
            }
        }
        let Some(colon) = top_level(&tokens[at..], |t| symbol(t, ":")).map(|c| c + at) else {
            return Err(Error::at(line, expected));
        };
        let range = match tokens.get(at).map(word) {
            Some("where") if colon > at + 1 => {
                self.condition(line, &tokens[at + 1..colon], vars)?
            }
            _ if colon == at => Vec::new(),
            _ => return Err(Error::at(line, expected)),
        };
        if colon + 1 == tokens.len() {
            return Err(Error::at(line, expected));
        }
        let body_tokens = &tokens[colon + 1..];
        let known: BTreeSet<&String> = names.iter().collect();
        if let Some(var) = tokens[at..]
            .iter()
            .filter_map(variable)
            .find(|v| v != "_" && !known.contains(v) && !outer.contains(v))
        {
            return Err(Error::at(
                line,
                format!(
                    "`?{var}` isn't bound: list it after `{}`",
                    match quantifier {
                        Quantifier::Some => "some",
                        Quantifier::Every => "every",
                        Quantifier::No => "no",
                    }
                ),
            ));
        }
        let body = self.condition(line, body_tokens, vars)?;
        Ok(Claim::Quantified {
            quantifier,
            vars: names,
            range,
            body,
        })
    }

    fn pattern(
        &self,
        line: &Line,
        tokens: &[Token],
        vars: &mut BTreeMap<String, String>,
        after: &str,
    ) -> Result<Vec<Literal>, Error> {
        if tokens.is_empty() {
            return Err(Error::at(line, format!("`{after}` needs a sentence")));
        }
        self.condition(line, tokens, vars)
    }
}

fn unbound(line: &Line, var: &str) -> Error {
    Error::at(
        line,
        format!("`?{var}` isn't bound: it must appear in the event sentence, in `when`, or in an `is a new` line"),
    )
}

const HANDLER_LINE: &str = "a handler line is `when`, `then`, `ensures` or `may skip`";

fn old_verb(line: &Line, vars: &BTreeMap<String, String>) -> Option<Error> {
    let keyword = line.tokens.first().map(word)?;
    let rest = line.text.trim().strip_prefix(keyword)?.trim();
    let post = match keyword {
        "set" | "include" | "emit" => rest.to_string(),
        "withdraw" => format!("not {rest}"),
        "create" => match rest.split_once(':') {
            Some((var, ty)) => format!("{} is a new {}", var.trim(), ty.trim()),
            None => format!("{rest} is a new T"),
        },
        "delete" => {
            let ty = rest.strip_prefix('?').and_then(|v| vars.get(v)).map_or("T", String::as_str);
            format!("not {rest} is a {ty}")
        }
        _ => return None,
    };
    Some(Error::at(
        line,
        format!("`{keyword}` is gone: a handler states what holds afterwards, so write\n  then\n    {post}"),
    ))
}

fn new_fields(line: &Line, record: &[Token]) -> Result<Vec<(String, Expr)>, Error> {
    let expected = "`is a new T` takes its fields as a record, as in `?row is a new MenuRow {key: \"a\", order: 1}`";
    let ExprKind::Record(fields) = parse_expr(record, line)?.kind else {
        return Err(Error::at(line, expected));
    };
    let mut out: Vec<(String, Expr)> = Vec::new();
    for field in fields {
        let [Segment::Name(name)] = field.path.segments.as_slice() else {
            return Err(Error::at(line, expected));
        };
        if out.iter().any(|(n, _)| n == name) {
            return Err(Error::at(line, format!("field `{name}` is given twice")));
        }
        out.push((name.clone(), field.value));
    }
    Ok(out)
}

fn phrase(tokens: &[Token], words: &[&str]) -> Option<usize> {
    let mut depth = 0i32;
    for i in 0..tokens.len() {
        match word(&tokens[i]) {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            _ if depth == 0
                && tokens.len() >= i + words.len()
                && tokens[i..i + words.len()]
                    .iter()
                    .zip(words)
                    .all(|(t, w)| matches!(&t.kind, TokenKind::Word(x) if x == w)) =>
            {
                return Some(i)
            }
            _ => {}
        }
    }
    None
}
