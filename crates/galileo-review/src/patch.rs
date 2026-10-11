use crate::Baseline;
use galileo_core::ir::{Claim, Expr, FactOrigin, Literal, Program, RelationId, StateClaim};
use galileo_core::why::sentence;
use galileo_syntax::lower::{lower, lower_contract};
use galileo_syntax::{blocks, lex, units, Arg, Block, Error, Line, TokenKind};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Declaration {
    Rule,
    Definition,
    Handler,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    Add { line: usize },
    Withdraw { line: usize, baseline_line: usize },
    AddRule { name: String, line: usize },
    Replace { declaration: Declaration, name: String, line: usize },
    Remove { name: String, line: usize },
    ChangeRequirements { header: String, line: usize },
}

#[derive(Clone, Debug)]
pub struct Patch {
    pub name: String,
    pub edits: Vec<Edit>,
    pub edit_sentences: Vec<String>,
    pub source: String,
    pub program: Program,
    /// The patched program with the `intends` state claims as its only requirements.
    pub contract: Option<Program>,
    pub intends: Vec<Intent>,
    pub says: Option<String>,
    pub diagnostics: Vec<Error>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intent {
    Claim(usize),
    Only { relation: RelationId, pattern: Vec<Expr> },
    RequirementsUnchanged,
}

#[derive(Clone)]
enum Item {
    Claim(Line),
    Only(Line),
    RequirementsUnchanged,
}

#[derive(Clone, Copy)]
enum Origin {
    Baseline(usize),
    Patch(usize),
}

const SECTIONS: &str = "a patch section is `add`, `remove`, `add rule <name>`, `replace rule|definition|handler <name>`, `remove <name>`, `replace requirements`, `says` or `intends`";

const DECLARATIONS: [&str; 21] = [
    "type", "given", "state", "derived", "event", "effect", "rule", "on", "requirements",
    "define", "view", "nix", "foreign", "use", "patch", "private", "initially", "why", "trace",
    "prevent", "achieve",
];

pub fn parse(baseline: &Baseline, source: &str) -> Result<Patch, Error> {
    let top = blocks(source)?;
    let mut patch = None;
    for block in &top {
        if word(&block.line, 0) != Some("patch") || patch.is_some() {
            return Err(Error::at(
                &block.line,
                "this lies outside the patch: a patch file holds one `patch <name>` block, and declarations go in its sections",
            ));
        }
        patch = Some(block);
    }
    let Some(patch) = patch else {
        return Err(Error::bare("a patch is a `patch <name>` block"));
    };
    let name = match words(&patch.line).as_slice() {
        [_, name] => name.to_string(),
        _ => return Err(Error::at(&patch.line, "a patch is named: `patch library-ladder`")),
    };

    let base_top = blocks(baseline.source())?;
    let raw: Vec<&str> = source.split_terminator('\n').collect();
    let mut edit = Editing {
        baseline,
        base_top: &base_top,
        raw: &raw,
        deleted: BTreeSet::new(),
        appended: Vec::new(),
        edits: Vec::new(),
        intends: None,
        says: None,
    };
    for section in &patch.children {
        edit.section(section)?;
    }
    edit.prune(&base_top);

    let mut text = String::new();
    let mut origins = Vec::new();
    for (index, line) in baseline.source().split_terminator('\n').enumerate() {
        if !edit.deleted.contains(&(index + 1)) {
            text.push_str(line);
            text.push('\n');
            origins.push(Origin::Baseline(index + 1));
        }
    }
    for (line, no) in &edit.appended {
        text.push_str(line);
        text.push('\n');
        origins.push(Origin::Patch(*no));
    }
    let relocate = |e: Error| relocate_in(&origins, e);
    let lowered = lower(&text, "patch").map_err(relocate)?;
    let mut diagnostics: Vec<Error> = lowered.diagnostics.into_iter().map(relocate).collect();
    let mut intends = Vec::new();
    let contract = match &edit.intends {
        Some(items) => {
            let first = origins.len() + 1;
            let mut origins = origins.clone();
            let mut numbered = Vec::new();
            for item in items {
                if let Item::Claim(line) | Item::Only(line) = item {
                    origins.push(Origin::Patch(line.no));
                    numbered.push(Line { no: first + numbered.len(), ..line.clone() });
                }
            }
            let relocate = |e: Error| relocate_in(&origins, e);
            let lowered = lower_contract(&text, "patch", &numbered).map_err(relocate)?;
            diagnostics.extend(lowered.diagnostics.into_iter().filter(|e| e.line >= first).map(relocate));
            let mut contract = lowered.program;
            let at = |no: usize| contract.requirements.iter().position(|r| r.source.as_ref().is_some_and(|s| s.line == no));
            let mut no = first;
            for item in items {
                match item {
                    Item::RequirementsUnchanged => intends.push(Intent::RequirementsUnchanged),
                    Item::Claim(_) => {
                        intends.push(Intent::Claim(no));
                        no += 1;
                    }
                    Item::Only(line) => {
                        let Some(index) = at(no) else {
                            no += 1;
                            continue;
                        };
                        let atom = match &contract.requirements[index].claim {
                            Claim::State(StateClaim::Holds(atom)) => Some(atom),
                            Claim::State(StateClaim::Some { body, .. }) => body.iter().find_map(|l| match l {
                                Literal::Positive(atom) => Some(atom),
                                _ => None,
                            }),
                            _ => None,
                        };
                        let Some(atom) = atom.filter(|a| a.arguments.iter().all(|e| matches!(e, Expr::Value(_) | Expr::Variable(_)))) else {
                            return Err(Error::at(line, "`only P changes` takes one sentence, whose arguments are names or variables"));
                        };
                        intends.push(Intent::Only { relation: atom.relation, pattern: atom.arguments.clone() });
                        no += 1;
                    }
                }
            }
            let kept: Vec<usize> = intends.iter().filter_map(|i| match i { Intent::Claim(no) => at(*no), _ => None }).collect();
            for intent in &mut intends {
                if let Intent::Claim(no) = intent {
                    *no = at(*no).and_then(|index| kept.iter().position(|k| *k == index)).unwrap_or(usize::MAX);
                }
            }
            let requirements = std::mem::take(&mut contract.requirements);
            contract.requirements = kept.iter().map(|index| requirements[*index].clone()).collect();
            Some(contract)
        }
        None => None,
    };
    let edit_sentences = render_edits(
        baseline,
        patch,
        &edit.edits,
        &lowered.program,
        &origins,
    );
    Ok(Patch {
        name,
        edits: edit.edits,
        edit_sentences,
        source: text,
        program: lowered.program,
        contract,
        intends,
        says: edit.says,
        diagnostics,
    })
}

fn relocate_in(origins: &[Origin], mut e: Error) -> Error {
    match origins.get(e.line.wrapping_sub(1)) {
        Some(Origin::Patch(no)) => e.line = *no,
        Some(Origin::Baseline(no)) => {
            e.message = format!("baseline line {no}, after this patch: {}", e.message);
            e.line = 0;
        }
        None => {}
    }
    e
}

fn render_edits(
    baseline: &Baseline,
    patch: &Block,
    edits: &[Edit],
    program: &Program,
    origins: &[Origin],
) -> Vec<String> {
    let mut sentences = Vec::new();
    for edit in edits {
        match edit {
            Edit::Add { line } => {
                let block = patch
                    .children
                    .iter()
                    .flat_map(|section| &section.children)
                    .find(|block| block.line.no == *line)
                    .expect("an addition comes from a patch block");
                let end = last(block);
                let added = |no: usize| match origins.get(no.wrapping_sub(1)) {
                    Some(Origin::Patch(no)) if (*line..=end).contains(no) => Some(*no),
                    _ => None,
                };
                for fact in program.facts.iter().chain(&program.initially) {
                    if let FactOrigin::Stated(Some(span)) = &fact.origin {
                        if let Some(no) = added(span.line) {
                            sentences.push(format!(
                                "Added: {}    patch:{no}",
                                sentence(program, fact.relation, &fact.values)
                            ));
                        }
                    }
                }
            }
            Edit::Withdraw {
                line,
                baseline_line,
            } => {
                let parsed = baseline.sentences();
                for fact in parsed.facts.iter().filter(|fact| fact.line == *baseline_line) {
                    sentences.push(format!(
                        "Withdrew: {}    patch:{line}",
                        parsed.vocabulary.sentence(fact.shape, &fact.args)
                    ));
                }
            }
            Edit::AddRule { name, line } => {
                sentences.push(format!("Added rule {name}    patch:{line}"));
            }
            Edit::Replace {
                declaration,
                name,
                line,
            } => {
                let kind = match declaration {
                    Declaration::Rule => "rule",
                    Declaration::Definition => "definition",
                    Declaration::Handler => "handler",
                };
                sentences.push(format!("Replaced {kind} {name}    patch:{line}"));
            }
            Edit::Remove { name, line } => {
                let kind = if baseline.program().rules.iter().any(|rule| rule.name == *name) {
                    "rule"
                } else {
                    "definition"
                };
                sentences.push(format!("Removed {kind} {name}    patch:{line}"));
            }
            Edit::ChangeRequirements { header, line } => {
                sentences.push(format!("Changed {header}    patch:{line}"));
            }
        }
    }
    sentences
}

struct Editing<'a> {
    baseline: &'a Baseline,
    base_top: &'a [Block],
    raw: &'a [&'a str],
    deleted: BTreeSet<usize>,
    appended: Vec<(String, usize)>,
    edits: Vec<Edit>,
    intends: Option<Vec<Item>>,
    says: Option<String>,
}

impl<'a> Editing<'a> {
    fn section(&mut self, section: &Block) -> Result<(), Error> {
        let line = &section.line;
        let no = line.no;
        match words(line).as_slice() {
            ["intends"] => {
                if self.intends.is_some() {
                    return Err(Error::at(line, "a patch has one `intends`"));
                }
                if section.children.is_empty() {
                    return Err(Error::at(line, "`intends` needs its claims indented beneath, one per line"));
                }
                let mut items = Vec::new();
                for child in &section.children {
                    if let Some(deeper) = child.children.first() {
                        return Err(Error::at(&deeper.line, "an intended claim is one line"));
                    }
                    items.push(intent(&child.line)?);
                }
                self.intends = Some(items);
            }
            ["says", _] => {
                if self.says.is_some() {
                    return Err(Error::at(line, "a patch has one `says`"));
                }
                let Some(TokenKind::Text(text)) = line.tokens.get(1).map(|t| &t.kind) else {
                    return Err(Error::at(line, "`says` takes one text: `says \"…\"`"));
                };
                self.says = Some(text.clone());
            }
            ["add"] => {
                for child in &section.children {
                    self.check_addition(child)?;
                }
                let state = self.baseline.sentences().vocabulary.state_lines(&section.children).unwrap_or_default();
                for child in &section.children {
                    self.place(child, &state);
                    self.edits.push(Edit::Add { line: child.line.no });
                }
            }
            ["remove"] => {
                for child in &section.children {
                    self.withdraw(child)?;
                }
            }
            ["add", "rule", name] => {
                if self.find(Declaration::Rule, name).is_some() {
                    return Err(Error::at(line, format!("the baseline already has rule `{name}`; change it with `replace rule {name}`")));
                }
                self.append_rule(section, name)?;
                self.edits.push(Edit::AddRule { name: name.to_string(), line: no });
            }
            ["replace", "handler", ..] => {
                let event = line.text.trim_start()["replace handler".len()..].trim();
                self.replace(section, Declaration::Handler, event)?;
            }
            ["replace", kind, name] => {
                let declaration = match *kind {
                    "rule" => Declaration::Rule,
                    "definition" => Declaration::Definition,
                    _ => return Err(Error::at(line, SECTIONS)),
                };
                self.replace(section, declaration, name)?;
            }
            ["remove", name] => {
                let found = [Declaration::Rule, Declaration::Definition]
                    .into_iter()
                    .find_map(|d| self.find(d, name));
                let Some(block) = found else {
                    return Err(Error::at(line, format!("the baseline has no rule or definition `{name}` to remove")));
                };
                if !section.children.is_empty() {
                    return Err(Error::at(&section.children[0].line, "`remove <name>` takes nothing beneath it"));
                }
                self.delete(block);
                self.edits.push(Edit::Remove { name: name.to_string(), line: no });
            }
            ["replace", "requirements"] => {
                for child in &section.children {
                    if word(&child.line, 0) != Some("requirements") {
                        return Err(Error::at(&child.line, "`replace requirements` holds `requirements` blocks only"));
                    }
                    let header = child.line.text.clone();
                    if let Some(old) = self.base_top.iter().find(|b| b.line.text == header) {
                        self.delete(old);
                    }
                    self.append_block(child, child.line.indent);
                    self.edits.push(Edit::ChangeRequirements { header, line: child.line.no });
                }
            }
            _ => return Err(Error::at(line, SECTIONS)),
        }
        Ok(())
    }

    fn check_addition(&self, block: &Block) -> Result<(), Error> {
        if let Some(first) = word(&block.line, 0).filter(|w| DECLARATIONS.contains(w)) {
            return Err(Error::at(
                &block.line,
                format!("`add` takes facts and entities, not `{first}`; a patch cannot declare outside its sections"),
            ));
        }
        if let [name, "is", "a", _] = words(&block.line).as_slice() {
            if self.baseline.sentences().vocabulary.entities.contains_key(*name) {
                return Err(Error::at(&block.line, format!("`{name}` is already a baseline entity")));
            }
        }
        Ok(())
    }

    fn withdraw(&mut self, block: &Block) -> Result<(), Error> {
        let subject = units(&block.line, &block.line.tokens)?;
        let mut sentences = Vec::new();
        if block.children.is_empty() {
            sentences.push((block.line.clone(), subject.clone()));
        }
        for child in &block.children {
            let mut sentence = subject.clone();
            sentence.extend(units(&child.line, &child.line.tokens)?);
            sentences.push((child.line.clone(), sentence));
        }
        let sentences_of = self.baseline.sentences();
        for (line, sentence) in sentences {
            let reading = sentences_of.vocabulary.read(&line, &sentence, None)?;
            if reading.args.iter().any(|a| matches!(a, Arg::List(_))) {
                return Err(Error::at(&line, "remove one fact per line"));
            }
            let Some(fact) = sentences_of
                .facts
                .iter()
                .find(|f| f.shape == reading.shape && f.args == reading.args)
            else {
                return Err(Error::at(&line, "the baseline does not state this fact"));
            };
            if sentences_of.facts.iter().filter(|f| f.line == fact.line).count() > 1 {
                return Err(Error::at(
                    &line,
                    format!("baseline line {} states several facts at once; remove them all", fact.line),
                ));
            }
            self.deleted.insert(fact.line);
            self.edits.push(Edit::Withdraw { line: line.no, baseline_line: fact.line });
        }
        Ok(())
    }

    fn replace(&mut self, section: &Block, declaration: Declaration, name: &str) -> Result<(), Error> {
        let line = &section.line;
        let Some(old) = self.find(declaration, name) else {
            return Err(Error::at(line, format!("the baseline has no {declaration:?} `{name}` to replace").to_lowercase()));
        };
        self.delete(old);
        match declaration {
            Declaration::Rule => self.append_rule(section, name)?,
            Declaration::Handler => self.append_children(section, format!("on {name}")),
            Declaration::Definition => {
                let [new] = section.children.as_slice() else {
                    return Err(Error::at(line, "`replace definition <name>` holds one `define` block"));
                };
                if word(&new.line, 0) != Some("define") || word(&new.line, 1) != Some(name) {
                    return Err(Error::at(&new.line, format!("expected `define {name}(…)`")));
                }
                self.append_block(new, new.line.indent);
            }
        }
        self.edits.push(Edit::Replace { declaration, name: name.into(), line: line.no });
        Ok(())
    }

    fn find(&self, declaration: Declaration, name: &str) -> Option<&'a Block> {
        { let top: &'a [Block] = self.base_top; top }.iter().find(|b| match declaration {
            Declaration::Rule => word(&b.line, 0) == Some("rule") && word(&b.line, 1) == Some(name),
            Declaration::Definition => {
                word(&b.line, 0) == Some("define") && word(&b.line, 1) == Some(name)
            }
            Declaration::Handler => {
                word(&b.line, 0) == Some("on") && b.line.text["on".len()..].trim() == name
            }
        })
    }

    fn delete(&mut self, block: &Block) {
        self.deleted.extend(block.line.no..=last(block));
    }

    fn prune(&mut self, top: &[Block]) {
        fn emptied(block: &Block, deleted: &mut BTreeSet<usize>) -> bool {
            if block.children.is_empty() {
                return deleted.contains(&block.line.no);
            }
            let mut all = true;
            for child in &block.children {
                all &= emptied(child, deleted);
            }
            if all {
                deleted.insert(block.line.no);
            }
            all
        }
        for block in top {
            emptied(block, &mut self.deleted);
        }
    }

    fn append_rule(&mut self, section: &Block, name: &str) -> Result<(), Error> {
        let Some((head, conditions)) = section.children.split_first() else {
            return Err(Error::at(&section.line, "a rule edit needs a head sentence"));
        };
        self.appended.push((format!("rule {name}: {}", head.line.text.trim()), section.line.no));
        for condition in conditions {
            self.append_block(condition, condition.line.indent - 2);
        }
        Ok(())
    }

    fn append_children(&mut self, section: &Block, header: String) {
        self.appended.push((header, section.line.no));
        for child in &section.children {
            self.append_block(child, child.line.indent - 2);
        }
    }

    fn append_block(&mut self, block: &Block, dedent: usize) {
        self.append_lines(block.line.no..=last(block), dedent, "");
    }

    fn append_lines(&mut self, lines: std::ops::RangeInclusive<usize>, dedent: usize, prefix: &str) {
        for no in lines {
            let raw = self.raw[no - 1];
            let cut = raw.bytes().take(dedent).take_while(|c| *c == b' ').count();
            self.appended.push((format!("{prefix}{}", &raw[cut..]), no));
        }
    }

    fn place(&mut self, block: &Block, state: &BTreeSet<usize>) {
        let dedent = block.line.indent;
        let stated = |b: &Block| (b.line.no..=last(b)).any(|no| state.contains(&no));
        let table = block.line.tokens.iter().any(|t| matches!(t.kind, TokenKind::Variable(_)));
        if block.children.is_empty() || table {
            if stated(block) {
                self.appended.push(("initially".into(), block.line.no));
                self.append_lines(block.line.no..=last(block), dedent, "  ");
            } else {
                self.append_block(block, dedent);
            }
            return;
        }
        let (states, givens): (Vec<&Block>, Vec<&Block>) = block.children.iter().partition(|c| stated(c));
        if !givens.is_empty() {
            self.append_lines(block.line.no..=block.line.no, dedent, "");
            for child in givens {
                self.append_block(child, dedent);
            }
        }
        if !states.is_empty() {
            self.appended.push(("initially".into(), block.line.no));
            self.append_lines(block.line.no..=block.line.no, dedent, "  ");
            for child in states {
                self.append_lines(child.line.no..=last(child), dedent, "  ");
            }
        }
    }
}

fn last(block: &Block) -> usize {
    block.children.last().map_or(block.line.no, last)
}

fn intent(line: &Line) -> Result<Item, Error> {
    let all = words(line);
    if all == ["requirements", "unchanged"] {
        return Ok(Item::RequirementsUnchanged);
    }
    if all.len() >= 3 && all[0] == "only" && all[all.len() - 1] == "changes" {
        let tokens = &line.tokens[1..line.tokens.len() - 1];
        let mut vars: Vec<&str> = Vec::new();
        for token in tokens {
            if let TokenKind::Variable(name) = &token.kind {
                if !name.is_empty() && name != "_" && !vars.contains(&name.as_str()) {
                    vars.push(name);
                }
            }
        }
        let base = line.tokens[0].span.start;
        let pattern = &line.text[tokens[0].span.start - base..tokens[tokens.len() - 1].span.end - base];
        let text = if vars.is_empty() {
            pattern.to_string()
        } else {
            format!("some {}: {pattern}", vars.iter().map(|v| format!("?{v}")).collect::<Vec<_>>().join(", "))
        };
        let lexed = lex(&format!("{text}\n"))?;
        let [synthetic] = lexed.as_slice() else {
            return Err(Error::at(line, "`only P changes` takes one sentence"));
        };
        return Ok(Item::Only(Line { no: line.no, ..synthetic.clone() }));
    }
    Ok(Item::Claim(line.clone()))
}

fn words(line: &Line) -> Vec<&str> {
    line.tokens
        .iter()
        .map(|t| match &t.kind {
            TokenKind::Word(w) | TokenKind::Symbol(w) => w.as_str(),
            _ => "",
        })
        .collect()
}

fn word(line: &Line, index: usize) -> Option<&str> {
    match &line.tokens.get(index)?.kind {
        TokenKind::Word(w) => Some(w),
        _ => None,
    }
}
