use crate::rules::{symbol, top_level, variable, word, Literal};
use crate::{units, Block, Error, Kind, Line, Reading, Token, TokenKind, Vocabulary};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Set(Reading),
    Include(Reading),
    Withdraw(Reading),
    Create { var: String, ty: String },
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
pub enum Achieve {
    Possible,
    Acceptable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Question {
    Why(Vec<Literal>),
    WhyNot {
        goal: Vec<Literal>,
        via: Option<String>,
    },
    Trace(Vec<Literal>),
    Prevent {
        goal: Vec<Literal>,
        using: Vec<usize>,
    },
    Achieve {
        mode: Achieve,
        goal: Vec<Literal>,
        using: Vec<usize>,
    },
}

pub(crate) const QUESTIONS: [&str; 4] = ["why", "trace", "prevent", "achieve"];

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
                    return Err(Error::at(l, "`when` comes first, before the actions"));
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
            _ => {
                nested("actions")?;
                if !handler.ensures.is_empty() {
                    return Err(Error::at(l, "actions come before `ensures`"));
                }
                let action = v.action(l, &mut vars, &mut bound)?;
                handler.actions.push((l.no, action));
            }
        }
    }
    if handler.actions.is_empty() && !handler.may_skip {
        return Err(Error::at(
            line,
            "a handler needs at least one action: `set`, `include`, `withdraw`, `create`, `delete` or `emit`",
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

pub(crate) fn read_question(v: &Vocabulary, block: &Block) -> Result<Question, Error> {
    let line = &block.line;
    let tokens = &line.tokens;
    let first = word(&tokens[0]);
    let mut using = None;
    for child in &block.children {
        let l = &child.line;
        if !matches!(first, "prevent" | "achieve") || using.is_some() {
            return Err(Error::at(
                l,
                "only `prevent` and `achieve` take a `using:` line",
            ));
        }
        using = Some(v.using(l)?);
    }
    let needs_using = |using: Option<Vec<usize>>| {
        using.ok_or_else(|| {
            Error::at(
                line,
                format!("`{first}` needs a `using:` line naming the shapes it may change"),
            )
        })
    };
    let goal = |rest: &[Token]| {
        if rest.is_empty() {
            return Err(Error::at(
                line,
                format!("`{first}` is followed by the sentence it asks about"),
            ));
        }
        v.condition(line, rest, &mut BTreeMap::new())
    };
    Ok(match first {
        "why" if tokens.get(1).map(word) == Some("not") => {
            let rest = &tokens[2..];
            match top_level(rest, |t| word(t) == "via") {
                Some(at) => match &rest[at + 1..] {
                    [rule] if matches!(rule.kind, TokenKind::Word(_)) => Question::WhyNot {
                        goal: goal(&rest[..at])?,
                        via: Some(word(rule).into()),
                    },
                    _ => {
                        return Err(Error::at(
                            line,
                            "`via` names one rule, as in `why not S via held-keys-open`",
                        ))
                    }
                },
                None => Question::WhyNot {
                    goal: goal(rest)?,
                    via: None,
                },
            }
        }
        "why" => Question::Why(goal(&tokens[1..])?),
        "trace" => Question::Trace(goal(&tokens[1..])?),
        "prevent" => Question::Prevent {
            goal: goal(&tokens[1..])?,
            using: needs_using(using)?,
        },
        _ => {
            let (mode, rest) = match tokens.get(1).map(word) {
                Some("possible") => (Achieve::Possible, &tokens[2..]),
                Some("acceptable") => (Achieve::Acceptable, &tokens[2..]),
                _ => {
                    return Err(Error::at(
                        line,
                        "say which `achieve`: `achieve possible S` ignores requirements, `achieve acceptable S` keeps them",
                    ))
                }
            };
            Question::Achieve {
                mode,
                goal: goal(rest)?,
                using: needs_using(using)?,
            }
        }
    })
}

impl Vocabulary {
    fn using(&self, line: &Line) -> Result<Vec<usize>, Error> {
        let expected = "write `using: shape-name, shape-name`";
        let [first, colon, rest @ ..] = line.tokens.as_slice() else {
            return Err(Error::at(line, expected));
        };
        if word(first) != "using" || !symbol(colon, ":") || rest.is_empty() {
            return Err(Error::at(line, expected));
        }
        let mut out = Vec::new();
        for (i, token) in rest.iter().enumerate() {
            match (i % 2, &token.kind) {
                (0, TokenKind::Word(name)) => {
                    let index = self
                        .shapes
                        .iter()
                        .position(|s| &s.name == name)
                        .ok_or_else(|| {
                            Error::at(
                                line,
                                format!(
                                    "`{name}` isn't a shape in scope; `using:` lists shape names"
                                ),
                            )
                        })?;
                    out.push(index);
                }
                (1, _) if symbol(token, ",") => {}
                _ => return Err(Error::at(line, expected)),
            }
        }
        Ok(out)
    }

    fn action(
        &self,
        line: &Line,
        vars: &mut BTreeMap<String, String>,
        bound: &mut BTreeSet<String>,
    ) -> Result<Action, Error> {
        let tokens = &line.tokens;
        let keyword = word(&tokens[0]);
        let rest = &tokens[1..];
        match (keyword, rest) {
            ("create", [var, colon, ty]) if symbol(colon, ":") && variable(var).is_some() => {
                let var = variable(var).unwrap();
                let ty = word(ty);
                if !self.types.get(ty).is_some_and(Option::is_none) {
                    return Err(Error::at(line, format!("`create` makes an entity, but `{ty}` isn't an entity type")));
                }
                vars.insert(var.clone(), ty.into());
                bound.insert(var.clone());
                return Ok(Action::Create { var, ty: ty.into() });
            }
            ("create", _) => return Err(Error::at(line, "`create` is written `create ?row: MenuRow`")),
            ("delete", [var]) if variable(var).is_some() => {
                let var = variable(var).unwrap();
                if !bound.contains(&var) {
                    return Err(unbound(line, &var));
                }
                let ty = vars.get(&var).cloned().unwrap_or_default();
                return Ok(Action::Delete { var, ty });
            }
            ("delete", _) => return Err(Error::at(line, "`delete` takes one variable, as in `delete ?row`")),
            ("set" | "include" | "withdraw" | "emit", []) => {
                return Err(Error::at(line, format!("`{keyword}` is followed by a sentence")))
            }
            ("set" | "include" | "withdraw" | "emit", _) => {}
            _ => {
                return Err(Error::at(
                    line,
                    "a handler line is `when`, an action (`set`, `include`, `withdraw`, `create`, `delete`, `emit`), `ensures` or `may skip`",
                ))
            }
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
        let wanted = if keyword == "emit" {
            Kind::Effect
        } else {
            Kind::State
        };
        if shape.kind != wanted {
            let what = if keyword == "emit" {
                "an effect"
            } else {
                "a state"
            };
            return Err(Error::at(
                line,
                format!(
                    "`{keyword}` takes {what} sentence, but `{}` is {:?}",
                    shape.name, shape.kind
                ),
            ));
        }
        Ok(match keyword {
            "set" => {
                if shape.modes.is_empty() {
                    return Err(Error::at(
                        line,
                        format!(
                            "`set` replaces the value under a key, but `{}` has no `exactly one` or `at most one` line; use `include`",
                            shape.name
                        ),
                    ));
                }
                Action::Set(reading)
            }
            "include" => Action::Include(reading),
            "withdraw" => Action::Withdraw(reading),
            _ => Action::Emit(reading),
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
        format!("`?{var}` isn't bound: it must appear in the event sentence, in `when`, or in a `create`"),
    )
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
