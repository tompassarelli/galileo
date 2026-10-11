use crate::expr::{parse_expr, Expr, ExprKind, Query};
use crate::record::{Entry, Segment};
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
    Every { vars: Vec<String>, when: Vec<Literal>, post: Box<Action> },
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
    let mut arguments = BTreeSet::new();
    if !event.args.iter().all(|arg| {
        matches!(arg, crate::Arg::Var(name) if name != "_" && arguments.insert(name.clone()))
    }) {
        return Err(Error::at(
            line,
            "each argument on an `on` line needs its own distinct named variable; test values under `when`",
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
    let mut children = block.children.iter().enumerate().peekable();
    while let Some((index, child)) = children.next() {
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
            ("when" | "then", 1) => {
                let example = child.children.first().map(|b| b.line.text.trim()).unwrap_or(
                    if first == "when" { "?hero is in ?here" } else { "?hero is in ?there" },
                );
                let continuation = if first == "when" { "condition" } else { "post" };
                return Err(Error::at(l, format!("write `{first} {example}` on one line, then one `and {continuation}` per line")));
            }
            ("when" | "and", _) if handler.actions.is_empty() => {
                if (first == "when" && index != 0) || (first == "and" && handler.when.is_empty()) || !handler.ensures.is_empty() {
                    return Err(Error::at(l, "`when condition` comes first, then `and condition` before `then`"));
                }
                nested("conditions")?;
                if l.tokens.len() < 2 {
                    return Err(Error::at(l, "write `and condition` on one line"));
                }
                let tokens = &l.tokens[1..];
                let literal = v.line_literal(l, tokens, &mut vars)?;
                ground(&literal, &mut bound);
                handler.when.push((l.no, literal));
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
            ("then", _) => {
                if !handler.actions.is_empty() {
                    return Err(Error::at(l, "a handler has one `then`"));
                }
                if !handler.ensures.is_empty() {
                    return Err(Error::at(l, "`then` comes before `ensures`"));
                }
                let mut posts = vec![child];
                while children.peek().is_some_and(|(_, next)| next.line.tokens.first().map(word) == Some("and")) {
                    posts.push(children.next().unwrap().1);
                }
                for post in &posts {
                    if let Some(deeper) = post.children.first() {
                        return Err(Error::at(&deeper.line, "`then` lines don't nest"));
                    }
                    if post.line.tokens.len() < 2 {
                        return Err(Error::at(&post.line, "write `and post` on one line"));
                    }
                    let mut line = post.line.clone();
                    let prefix = word(&line.tokens[0]);
                    line.text = line.text.trim().strip_prefix(prefix).unwrap().trim_start().into();
                    line.tokens.remove(0);
                    let action = v.post(&line, &mut vars, &bound)?;
                    handler.actions.push((post.line.no, action));
                }
                let mut created = BTreeSet::new();
                for (post, (_, action)) in posts.iter().zip(&handler.actions) {
                    if let Action::Create { var, .. } = action {
                        if !created.insert(var.clone()) {
                            return Err(Error::at(&post.line, format!("`?{var}` is created twice in this clause")));
                        }
                    }
                }
                bound.extend(created.iter().cloned());
                let outside: BTreeSet<String> = bound.iter().chain(vars.keys()).cloned().collect();
                for (post, (_, action)) in posts.iter().zip(&handler.actions) {
                    let l = &post.line;
                    let (action, local) = match action {
                        Action::Every { vars: declared, when, post } => {
                            if let Some(name) = declared.iter().find(|name| outside.contains(*name)) {
                                return Err(Error::at(l, format!("`every ?{name}` declares `?{name}`, but the clause already names it; pick a name of its own")));
                            }
                            let mut local: BTreeSet<String> = bound.difference(&created).cloned().collect();
                            for literal in when { ground(literal, &mut local); }
                            let mut read = BTreeSet::new();
                            post_vars(post, &mut read);
                            if let Some(name) = read.iter().find(|name| created.contains(*name)) {
                                return Err(Error::at(l, format!("`?{name}` is bound by an `is a new` line, so an `every` post can't read it")));
                            }
                            for literal in when { v.check_literal(l, literal, &local)?; }
                            (post.as_ref(), local)
                        }
                        action => (action, bound.clone()),
                    };
                    let bound = &local;
                    match action {
                        Action::Create { fields, .. } => {
                            for (_, value) in fields {
                                v.check_expr(l, value, bound)?;
                            }
                        }
                        Action::Delete { var, .. } => {
                            if created.contains(var) {
                                return Err(Error::at(l, format!("`?{var}` is created by this clause; `not ?{var} is a T` deletes only an entity bound by `on` or `when`")));
                            }
                            if !bound.contains(var) {
                                return Err(unbound(l, var));
                            }
                        }
                        Action::Set(r) | Action::Include(r) | Action::Withdraw(r) | Action::Emit(r) => {
                            for arg in &r.args {
                                v.check_arg(l, arg, bound)?;
                            }
                        }
                        Action::Every { .. } => unreachable!("an `every` post holds no other"),
                    }
                }
            }
            _ => return Err(old_verb(l, &vars).unwrap_or_else(|| Error::at(l, HANDLER_LINE))),
        }
    }
    if handler.actions.is_empty() {
        return Err(Error::at(
            line,
            "a handler states what holds afterwards with `then ?hero is in ?there`",
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
        bound: &BTreeSet<String>,
    ) -> Result<Action, Error> {
        if let Some(error) = old_verb(line, vars) {
            return Err(error);
        }
        let tokens = line.tokens.as_slice();
        let is = |t: &Token, w: &str| matches!(&t.kind, TokenKind::Word(x) if x == w);
        if tokens.first().is_some_and(|t| is(t, "every")) {
            return self.every(line, &tokens[1..], vars, bound);
        }
        if let [var, i, a, n, ty, record @ ..] = tokens {
            if variable(var).is_some() && is(i, "is") && is(a, "a") && is(n, "new") {
                let var = variable(var).unwrap();
                let ty = word(ty);
                if !self.entity_shape(ty).is_some_and(|shape| self.shapes[shape].kind == Kind::State) {
                    return Err(Error::at(line, format!("`is a new` needs a `state entity`, but `{ty}` isn't one")));
                }
                if var == "_" || bound.contains(&var) {
                    return Err(Error::at(line, format!("`?{var}` is already bound by this clause; `is a new` needs a fresh named variable")));
                }
                let fields = if record.is_empty() {
                    Vec::new()
                } else {
                    new_fields(line, record)?
                };
                vars.insert(var.clone(), ty.into());
                return Ok(Action::Create { var, ty: ty.into(), fields });
            }
        }
        if let [not, var, i, a, ty] = tokens {
            if is(not, "not") && variable(var).is_some() && is(i, "is") && is(a, "a") {
                let var = variable(var).unwrap();
                let ty = word(ty);
                if !self.entity_shape(ty).is_some_and(|shape| self.shapes[shape].kind == Kind::State) {
                    return Err(Error::at(line, format!("`not ?{var} is a T` needs a `state entity`, but `{ty}` isn't one")));
                }
                let reading = self.read(line, &units(line, &tokens[1..])?, Some(vars))?;
                let ty = self.shapes[reading.shape].slots[0].ty.clone();
                return Ok(Action::Delete { var, ty });
            }
        }
        if let Some(mut reading) = self.field_reading(line, tokens, vars)? {
            let shape = &self.shapes[reading.shape];
            if shape.kind != Kind::State {
                return Err(Error::at(line, format!("`{}` is derived, so only rules give its facts", shape.name)));
            }
            if matches!(&reading.args[1], crate::Arg::Expr(text) if text == "null") {
                let field = &shape.slots[1].name;
                reading.args[1] = crate::Arg::Expr(format!("{}.{}", reading.args[0], field));
                return Ok(Action::Withdraw(reading));
            }
            return Ok(Action::Set(reading));
        }
        let withdraw = tokens.first().is_some_and(|t| is(t, "not"));
        let rest = &tokens[usize::from(withdraw)..];
        if rest.is_empty() {
            return Err(Error::at(line, "`not` is followed by the state sentence it withdraws"));
        }
        let reading = self.read(line, &units(line, rest)?, Some(vars))?;
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

    fn every(
        &self,
        line: &Line,
        tokens: &[Token],
        vars: &BTreeMap<String, String>,
        bound: &BTreeSet<String>,
    ) -> Result<Action, Error> {
        let expected = "an `every` post is `every ?v where <condition>: <post>`";
        let mut declared = Vec::new();
        let mut at = 0;
        while let Some(name) = tokens.get(at).and_then(variable) {
            declared.push(name);
            at += 1;
            if tokens.get(at).is_some_and(|t| symbol(t, ",")) {
                at += 1;
            } else {
                break;
            }
        }
        if declared.is_empty() {
            return Err(Error::at(line, expected));
        }
        let colon = top_level(&tokens[at..], |t| symbol(t, ":")).map(|c| c + at);
        let Some(colon) = colon.filter(|_| tokens.get(at).map(word) == Some("where")) else {
            return Err(Error::at(line, format!("{expected}: `every` needs `where` and the condition its variables range over")));
        };
        if colon == at + 1 || colon + 1 == tokens.len() {
            return Err(Error::at(line, expected));
        }
        let mut local = vars.clone();
        let when = self.condition(line, &tokens[at + 1..colon], &mut local)?;
        let body = &tokens[colon + 1..];
        if body.first().map(word) == Some("every") {
            return Err(Error::at(line, "one `every` holds no other; join their conditions under one `where`"));
        }
        let mut post_line = line.clone();
        post_line.tokens = body.to_vec();
        let post = self.post(&post_line, &mut local, bound)?;
        if let Action::Create { var, .. } = &post {
            return Err(Error::at(line, format!("an `every` post can't be an `is a new` line: write `?{var} is a new …` as its own `then` line")));
        }
        Ok(Action::Every { vars: declared, when, post: Box::new(post) })
    }

    fn check_arg(&self, line: &Line, arg: &crate::Arg, bound: &BTreeSet<String>) -> Result<(), Error> {
        match arg {
            crate::Arg::Var(name) if name != "_" && !bound.contains(name) => Err(unbound(line, name)),
            crate::Arg::List(items) => items.iter().try_for_each(|arg| self.check_arg(line, arg, bound)),
            crate::Arg::Text(text) if !text.contains('\n') => {
                let source = crate::lex(&format!("\"{text}\""))?;
                self.check_expr(line, &parse_expr(&source[0].tokens, line)?, bound)
            }
            crate::Arg::Expr(text) => {
                let source = crate::lex(text)?;
                self.check_expr(line, &parse_expr(&source[0].tokens, line)?, bound)
            }
            _ => Ok(()),
        }
    }

    fn check_literal(&self, line: &Line, literal: &Literal, bound: &BTreeSet<String>) -> Result<(), Error> {
        match literal {
            Literal::Sentence(r) | Literal::Not(r) => r.args.iter().try_for_each(|arg| self.check_arg(line, arg, bound)),
            Literal::Compare(_, a, b) => {
                self.check_expr(line, a, bound)?;
                self.check_expr(line, b, bound)
            }
            Literal::Holds(e) | Literal::Bind(_, e) | Literal::In(_, e) => self.check_expr(line, e, bound),
            Literal::Aggregate { of, body, .. } => {
                let mut local = bound.clone();
                for literal in body { ground(literal, &mut local); }
                for literal in body { self.check_literal(line, literal, &local)?; }
                of.as_ref().map_or(Ok(()), |expr| self.check_expr(line, expr, &local))
            }
            Literal::ArgMin { by: of, body, .. } => {
                let mut local = bound.clone();
                for literal in body {
                    ground(literal, &mut local);
                }
                for literal in body {
                    self.check_literal(line, literal, &local)?;
                }
                self.check_expr(line, of, &local)
            }
        }
    }

    fn check_expr(&self, line: &Line, expr: &Expr, bound: &BTreeSet<String>) -> Result<(), Error> {
        match &expr.kind {
            ExprKind::Var(name) if name != "_" && !bound.contains(name) => Err(unbound(line, name)),
            ExprKind::Record(fields) => fields.iter().try_for_each(|field| {
                let mut bound = bound.clone();
                for head in &field.each {
                    self.check_expr(line, &head.source, &bound)?;
                    bound.extend(head.vars.iter().cloned());
                }
                for segment in &field.path.segments {
                    if let Segment::Computed(expr) = segment { self.check_expr(line, expr, &bound)?; }
                }
                self.check_expr(line, &field.value, &bound)
            }),
            ExprKind::Query(query) => {
                let (conj, head, sorted_by) = match query {
                    Query::All { conj, head, sorted_by, .. } => (conj, head.as_deref(), sorted_by.as_deref()),
                    Query::The { conj, .. } => (conj, None, None),
                    Query::Aggregate { conj, value, .. } => (conj, value.as_deref(), None),
                };
                let body = self.condition(line, conj, &mut BTreeMap::new())?;
                let mut local = bound.clone();
                for literal in &body { ground(literal, &mut local); }
                for literal in &body { self.check_literal(line, literal, &local)?; }
                if let Some(head) = head { self.check_expr(line, head, &local)?; }
                sorted_by.map_or(Ok(()), |expr| self.check_expr(line, expr, &local))
            }
            ExprKind::NixModule(_, entries) => self.check_entries(line, entries, bound),
            _ => {
                let mut result = Ok(());
                crate::check::sub_exprs(expr, &mut |e| if result.is_ok() { result = self.check_expr(line, e, bound) });
                result
            }
        }
    }

    fn check_entries(&self, line: &Line, entries: &[Entry], bound: &BTreeSet<String>) -> Result<(), Error> {
        for entry in entries {
            let (path, when) = match entry {
                Entry::Value { path, value, when } => {
                    self.check_expr(line, value, bound)?;
                    (path, when)
                }
                Entry::Nested { path, when, entries } => {
                    self.check_entries(line, entries, bound)?;
                    (path, when)
                }
                Entry::For(block) => {
                    self.check_expr(line, &block.head.source, bound)?;
                    let mut local = bound.clone();
                    local.extend(block.head.vars.iter().cloned());
                    self.check_entries(line, &block.entries, &local)?;
                    continue;
                }
            };
            for segment in &path.segments {
                if let Segment::Computed(expr) = segment { self.check_expr(line, expr, bound)?; }
            }
            if let Some(expr) = when { self.check_expr(line, expr, bound)?; }
        }
        Ok(())
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
        let mut found = Vec::new();
        let mut failure = None;
        let mut local = vars.clone();
        match self.claim_form(line, tokens, &mut local) {
            Ok(claim) => found.push((claim, local, "claim".to_string())),
            Err(error) => failure = Some(error),
        }
        let mut keyword_failure = None;
        for words in [
            &["has", "no", "single", "point", "of", "failure", "among"][..],
            &["only", "after"][..],
            &["only", "when"][..],
            &["requires"][..],
        ] {
            let mut offset = 0;
            while let Some(at) = phrase(&tokens[offset..], words) {
                let at = offset + at;
                offset = at + words.len();
                let mut local = vars.clone();
                let reading = (|| {
                    let left = &tokens[..at];
                    let right = &tokens[offset..];
                    Ok(match words[0] {
                        "has" => {
                            let target = self.claim(line, left, &mut local)?;
                            let among = self.pattern(line, right, &mut local, "among")?;
                            if !matches!(among.as_slice(), [Literal::Sentence(_)]) {
                                let target = line.text.chars()
                                    .skip(left[0].span.column - line.indent - 1)
                                    .take(tokens[at].span.column - left[0].span.column)
                                    .collect::<String>();
                                let mut examples = String::new();
                                for literal in &among {
                                    if let Literal::Sentence(reading) = literal {
                                        examples.push_str(&format!(
                                            "\n  {} has no single point of failure among {}",
                                            target.trim_end(),
                                            self.sentence(reading.shape, &reading.args),
                                        ));
                                    }
                                }
                                return Err(Error::at(line, format!(
                                    "`among` takes one sentence; write one requirement per condition:{examples}"
                                )));
                            }
                            Claim::NoSinglePointOfFailure(Box::new(target), among)
                        }
                        "only" if words[1] == "after" => Claim::TracePasses(
                            self.pattern(line, left, &mut local, "only after")?,
                            self.pattern(line, right, &mut local, "only after")?,
                        ),
                        "only" => {
                            let event = self.read(line, &units(line, left)?, Some(&mut local))?;
                            let shape = &self.shapes[event.shape];
                            if shape.kind != Kind::Event {
                                return Err(Error::at(
                                    line,
                                    format!(
                                        "`only when` takes an event sentence, but `{}` is {:?}",
                                        shape.name, shape.kind
                                    ),
                                ));
                            }
                            Claim::HappensWhen(
                                event,
                                self.pattern(line, right, &mut local, "only when")?,
                            )
                        }
                        _ => {
                            let s = self.pattern(line, left, &mut local, "requires")?;
                            let c = self.pattern(line, right, &mut local, "requires")?;
                            if !matches!(s.as_slice(), [Literal::Sentence(_)])
                                || !matches!(c.as_slice(), [Literal::Sentence(_)])
                            {
                                let mut examples = String::new();
                                for target in &s {
                                    for condition in &c {
                                        if let (Literal::Sentence(target), Literal::Sentence(condition)) =
                                            (target, condition)
                                        {
                                            examples.push_str(&format!(
                                                "\n  {} requires {}",
                                                self.sentence(target.shape, &target.args),
                                                self.sentence(condition.shape, &condition.args),
                                            ));
                                        }
                                    }
                                }
                                return Err(Error::at(line, format!(
                                    "`requires` takes one sentence a side; write one requirement per condition:{examples}"
                                )));
                            }
                            Claim::Requires(s, c)
                        }
                    })
                })();
                match reading {
                    Ok(claim) => found.push((claim, local, words.join(" "))),
                    Err(error) if keyword_failure.is_none() => keyword_failure = Some(error),
                    Err(_) => {}
                }
            }
        }
        let mut readings = found
            .iter()
            .map(|(claim, _, keyword)| {
                let reading = match claim {
                    Claim::State(body) => self.condition_text(body),
                    Claim::Requires(left, right) | Claim::TracePasses(left, right) => format!(
                        "{} {keyword} {}",
                        self.condition_text(left),
                        self.condition_text(right)
                    ),
                    Claim::HappensWhen(event, body) => format!(
                        "{} {keyword} {}",
                        self.condition_text(&[Literal::Sentence(event.clone())]),
                        self.condition_text(body)
                    ),
                    other => format!("{keyword}: {other:?}"),
                };
                format!("  as {reading}")
            })
            .collect::<Vec<_>>();
        if let Some(ambiguous) = failure.as_ref().and_then(|error| {
            error
                .message
                .strip_prefix("this can be read more than one way:\n")
        }) {
            readings.push(ambiguous.to_string());
        }
        if readings.len() > 1 {
            return Err(Error::at(
                line,
                format!(
                    "this can be read more than one way:\n{}",
                    readings.join("\n")
                ),
            ));
        }
        match found.pop() {
            Some((claim, table, _)) => {
                *vars = table;
                Ok(claim)
            }
            None => Err(keyword_failure.or(failure).unwrap()),
        }
    }

    fn claim_form(
        &self,
        line: &Line,
        tokens: &[Token],
        vars: &mut BTreeMap<String, String>,
    ) -> Result<Claim, Error> {
        let rest = &tokens[1..];
        match word(&tokens[0]) {
            "possibly" => {
                return Ok(Claim::Eventually(self.pattern(
                    line,
                    rest,
                    vars,
                    "possibly",
                )?))
            }
            "eventually" => {
                if rest.is_empty() {
                    return Err(Error::at(line, "write `possibly S`: `eventually` means every run reaches S, which is not checked yet"));
                }
                let base = line.tokens[0].span.start;
                let condition = &line.text[rest[0].span.start - base..rest.last().unwrap().span.end - base];
                return Err(Error::at(
                    line,
                    format!("write `possibly {condition}`: `eventually` means every run reaches it, which is not checked yet"),
                ));
            }
            "always" => return Ok(Claim::Always(self.pattern(line, rest, vars, "always")?)),
            "never" => return Ok(Claim::Never(self.pattern(line, rest, vars, "never")?)),
            "before" => return Ok(Claim::Before(self.pattern(line, rest, vars, "before")?)),
            "every" if rest.first().map(word) == Some("trace") => {
                let expected = "write `S only after C`";
                if rest.get(1).map(word) != Some("to") {
                    return Err(Error::at(line, expected));
                }
                let Some(at) = phrase(&rest[2..], &["passes"]) else {
                    return Err(Error::at(line, expected));
                };
                let target = &rest[2..2 + at];
                let condition = &rest[3 + at..];
                if target.is_empty() || condition.is_empty() {
                    return Err(Error::at(line, expected));
                }
                let base = line.tokens[0].span.start;
                let target =
                    &line.text[target[0].span.start - base..target.last().unwrap().span.end - base];
                let condition = &line.text
                    [condition[0].span.start - base..condition.last().unwrap().span.end - base];
                return Err(Error::at(
                    line,
                    format!("write `{target} only after {condition}`"),
                ));
            }
            "every" if phrase(tokens, &["happens", "when"]).is_some() => {
                let at = phrase(tokens, &["happens", "when"]).unwrap();
                let event = &tokens[1..at];
                let condition = &tokens[at + 2..];
                if event.is_empty() || condition.is_empty() {
                    return Err(Error::at(line, "write `E only when C`"));
                }
                let base = line.tokens[0].span.start;
                let event =
                    &line.text[event[0].span.start - base..event.last().unwrap().span.end - base];
                let condition = &line.text
                    [condition[0].span.start - base..condition.last().unwrap().span.end - base];
                return Err(Error::at(line, format!("write `{event} only when {condition}`")));
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
        Ok(Claim::State(self.condition(line, tokens, vars)?))
    }

    fn quantified(
        &self,
        line: &Line,
        quantifier: Quantifier,
        tokens: &[Token],
        vars: &mut BTreeMap<String, String>,
    ) -> Result<Claim, Error> {
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

fn post_vars(action: &Action, read: &mut BTreeSet<String>) {
    fn arg(a: &crate::Arg, read: &mut BTreeSet<String>) {
        match a {
            crate::Arg::Var(name) => { read.insert(name.clone()); }
            crate::Arg::List(items) => items.iter().for_each(|i| arg(i, read)),
            crate::Arg::Expr(text) => read.extend(text.split(|c: char| !(c.is_alphanumeric() || c == '?' || c == '-' || c == '_')).filter_map(|t| t.strip_prefix('?')).map(String::from)),
            _ => {}
        }
    }
    match action {
        Action::Set(r) | Action::Include(r) | Action::Withdraw(r) | Action::Emit(r) => r.args.iter().for_each(|a| arg(a, read)),
        Action::Delete { var, .. } => { read.insert(var.clone()); }
        Action::Create { .. } | Action::Every { .. } => {}
    }
}

pub(crate) fn ground(literal: &Literal, bound: &mut BTreeSet<String>) {
    match literal {
        Literal::Sentence(reading) => {
            bound.extend(reading.args.iter().filter_map(|arg| match arg {
                crate::Arg::Var(name) if name != "_" => Some(name.clone()),
                _ => None,
            }));
        }
        Literal::Bind(var, _) | Literal::In(var, _) | Literal::Aggregate { var, .. } | Literal::ArgMin { var, .. } => {
            bound.insert(var.clone());
        }
        Literal::Not(_) | Literal::Compare(..) | Literal::Holds(_) => {}
    }
}

fn unbound(line: &Line, var: &str) -> Error {
    Error::at(
        line,
        format!("`?{var}` isn't bound: it must appear in the event sentence, in `when`, or in an `is a new` line"),
    )
}

const HANDLER_LINE: &str = "a handler line is `when condition`, `then post`, `and condition` or `and post`, `ensures` or `may skip`";

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
        format!("`{keyword}` is gone: a handler states what holds afterwards, so write\n  then {post}"),
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
