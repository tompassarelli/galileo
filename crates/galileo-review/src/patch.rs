use crate::Baseline;
use galileo_core::ir::{Claim, Expr, FactOrigin, Literal, Program, RelationId, StateClaim};
use galileo_core::why::sentence;
use galileo_syntax::lower::{lower, lower_contract, lower_program};
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
    Withdraw { line: usize, module: String, baseline_line: usize },
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
    /// Each edited baseline module's text by name, then the added module's when it holds an edit.
    pub modules: Vec<(String, String)>,
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
    Header,
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

    let modules = baseline.modules();
    let base_tops = modules.iter().map(|(_, text)| blocks(text)).collect::<Result<Vec<_>, _>>()?;
    let raw: Vec<&str> = source.split_terminator('\n').collect();
    let added = if baseline.added().is_some() { modules.len() } else { 0 };
    let mut edit = Editing {
        baseline,
        base_tops: &base_tops,
        raw: &raw,
        deleted: vec![BTreeSet::new(); modules.len()],
        appended: vec![Vec::new(); added + 1],
        target: added,
        added,
        edits: Vec::new(),
        intends: None,
        says: None,
    };
    for section in &patch.children {
        edit.section(section)?;
    }
    edit.prune(&base_tops);

    let mut files: Vec<(String, String, Vec<Origin>)> = Vec::new();
    for (index, (module, text)) in modules.iter().enumerate() {
        let mut edited = String::new();
        let mut origins = Vec::new();
        for (no, line) in text.split_terminator('\n').enumerate() {
            if !edit.deleted[index].contains(&(no + 1)) {
                edited.push_str(line);
                edited.push('\n');
                origins.push(Origin::Baseline(no + 1));
            }
        }
        files.push((module.clone(), edited, origins));
    }
    if let Some((module, _)) = baseline.added() {
        let header = crate::baseline::uses(modules.iter().map(|(name, _)| name.as_str()));
        let origins = header.lines().map(|_| Origin::Header).collect();
        files.push((module.clone(), header, origins));
    }
    for (line, no) in &edit.appended[added] {
        let (_, text, origins) = &mut files[added];
        text.push_str(line);
        text.push('\n');
        origins.push(Origin::Patch(*no));
    }
    for (index, appended) in edit.appended[..added].iter().enumerate() {
        let (_, text, origins) = &mut files[index];
        for (line, no) in appended {
            text.push_str(line);
            text.push('\n');
            origins.push(Origin::Patch(*no));
        }
    }
    let single = baseline.added().is_none();
    if single {
        files[0].0 = "patch".into();
    }
    let relocate = |e: Error| if single { relocate_in(&files[0].2, e) } else { relocate_named(&files, e) };
    let lowered = if single {
        lower(&files[0].1, "patch")
    } else {
        let named: Vec<(&str, &str)> = files.iter().map(|(name, text, _)| (name.as_str(), text.as_str())).collect();
        lower_program(&named)
    }
    .map_err(relocate)?;
    let mut diagnostics: Vec<Error> = lowered.diagnostics.into_iter().map(relocate).collect();
    let origins = files[0].2.clone();
    let text = files[added].1.clone();
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
        &files,
    );
    let mut edited: Vec<(String, String)> = files.iter().map(|(name, text, _)| (name.clone(), text.clone())).collect();
    if single {
        edited[0].0 = modules[0].0.clone();
    } else if edit.appended[added].is_empty() {
        edited.pop();
    }
    Ok(Patch {
        name,
        edits: edit.edits,
        edit_sentences,
        source: text,
        program: lowered.program,
        modules: edited,
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
        Some(Origin::Header) => e.line = 0,
        None => {}
    }
    e
}

fn relocate_named(files: &[(String, String, Vec<Origin>)], mut e: Error) -> Error {
    let Some((module, origins, rest)) = files
        .iter()
        .filter_map(|(name, _, origins)| Some((name, origins, e.message.strip_prefix(name.as_str())?.strip_prefix(": ")?)))
        .max_by_key(|(name, _, _)| name.len())
    else {
        return e;
    };
    let rest = rest.to_string();
    match origins.get(e.line.wrapping_sub(1)) {
        Some(Origin::Patch(no)) => {
            e.line = *no;
            e.message = rest;
        }
        Some(Origin::Baseline(no)) => {
            e.message = format!("{module}:{no}, after this patch: {rest}");
            e.line = 0;
        }
        Some(Origin::Header) => e.line = 0,
        None => {}
    }
    e
}

fn render_edits(
    baseline: &Baseline,
    patch: &Block,
    edits: &[Edit],
    program: &Program,
    files: &[(String, String, Vec<Origin>)],
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
                let added = |file: &str, no: usize| {
                    let (_, _, origins) = files.iter().find(|(name, _, _)| name == file)?;
                    match origins.get(no.wrapping_sub(1)) {
                        Some(Origin::Patch(no)) if (*line..=end).contains(no) => Some(*no),
                        _ => None,
                    }
                };
                for fact in program.facts.iter().chain(&program.initially) {
                    if let FactOrigin::Stated(Some(span)) = &fact.origin {
                        if let Some(no) = added(&span.file, span.line) {
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
                module,
                baseline_line,
            } => {
                let index = baseline.modules().iter().position(|(name, _)| name == module).expect("a withdrawal names a baseline module");
                let parsed = &baseline.module_sentences()[index];
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
    base_tops: &'a [Vec<Block>],
    raw: &'a [&'a str],
    deleted: Vec<BTreeSet<usize>>,
    appended: Vec<Vec<(String, usize)>>,
    target: usize,
    added: usize,
    edits: Vec<Edit>,
    intends: Option<Vec<Item>>,
    says: Option<String>,
}

impl<'a> Editing<'a> {
    fn section(&mut self, section: &Block) -> Result<(), Error> {
        let line = &section.line;
        let no = line.no;
        self.target = self.added;
        match words(line).as_slice() {
            ["intends"] if self.baseline.added().is_some() => {
                return Err(Error::at(line, "`intends` in a patch over several modules is not yet supported"));
            }
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
                let vocabulary = match self.baseline.added() {
                    Some((_, sentences)) => &sentences.vocabulary,
                    None => &self.baseline.sentences().vocabulary,
                };
                let state = vocabulary.state_lines(&section.children).unwrap_or_default();
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
                if !self.find(&[Declaration::Rule], name).is_empty() {
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
                let Some((module, block)) = self.unique(line, &[Declaration::Rule, Declaration::Definition], name)? else {
                    return Err(Error::at(line, format!("the baseline has no rule or definition `{name}` to remove")));
                };
                self.target = module;
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
                    self.target = self.added;
                    let tops: &'a [Vec<Block>] = self.base_tops;
                    let found = tops.iter().enumerate().find_map(|(m, top)| Some((m, top.iter().find(|b| b.line.text == header)?)));
                    if let Some((module, old)) = found {
                        self.target = module;
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
            if self.baseline.module_sentences().iter().any(|s| s.vocabulary.entities.contains_key(*name)) {
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
        for (line, sentence) in sentences {
            let mut found = false;
            let mut read_any = false;
            let mut unread = None;
            for (module, sentences_of) in self.baseline.module_sentences().iter().enumerate() {
                let reading = match sentences_of.vocabulary.read(&line, &sentence, None) {
                    Ok(reading) => reading,
                    Err(e) => {
                        unread.get_or_insert(e);
                        continue;
                    }
                };
                if reading.args.iter().any(|a| matches!(a, Arg::List(_))) {
                    return Err(Error::at(&line, "remove one fact per line"));
                }
                let Some(fact) = sentences_of
                    .facts
                    .iter()
                    .find(|f| f.shape == reading.shape && f.args == reading.args)
                else {
                    read_any = true;
                    continue;
                };
                if sentences_of.facts.iter().filter(|f| f.line == fact.line).count() > 1 {
                    return Err(Error::at(
                        &line,
                        format!("baseline line {} states several facts at once; remove them all", fact.line),
                    ));
                }
                found = true;
                self.deleted[module].insert(fact.line);
                let module = self.baseline.modules()[module].0.clone();
                self.edits.push(Edit::Withdraw { line: line.no, module, baseline_line: fact.line });
            }
            if !found {
                return Err(match unread.filter(|_| !read_any) {
                    Some(e) => e,
                    None => Error::at(&line, "the baseline does not state this fact"),
                });
            }
        }
        Ok(())
    }

    fn replace(&mut self, section: &Block, declaration: Declaration, name: &str) -> Result<(), Error> {
        let line = &section.line;
        let Some((module, old)) = self.unique(line, &[declaration], name)? else {
            return Err(Error::at(line, format!("the baseline has no {declaration:?} `{name}` to replace").to_lowercase()));
        };
        self.target = module;
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

    fn find(&self, declarations: &[Declaration], name: &str) -> Vec<(usize, &'a Block)> {
        let tops: &'a [Vec<Block>] = self.base_tops;
        let mut found = Vec::new();
        for (module, top) in tops.iter().enumerate() {
            for declaration in declarations {
                let block = top.iter().find(|b| match declaration {
                    Declaration::Rule => word(&b.line, 0) == Some("rule") && word(&b.line, 1) == Some(name),
                    Declaration::Definition => word(&b.line, 0) == Some("define") && word(&b.line, 1) == Some(name),
                    Declaration::Handler => word(&b.line, 0) == Some("on") && b.line.text["on".len()..].trim() == name,
                });
                if let Some(block) = block {
                    found.push((module, block));
                    break;
                }
            }
        }
        found
    }

    fn unique(&self, line: &Line, declarations: &[Declaration], name: &str) -> Result<Option<(usize, &'a Block)>, Error> {
        match self.find(declarations, name).as_slice() {
            [] => Ok(None),
            [one] => Ok(Some(*one)),
            [(a, _), (b, _), ..] => {
                let modules = self.baseline.modules();
                Err(Error::at(line, format!("`{name}` is declared in both {} and {}", modules[*a].0, modules[*b].0)))
            }
        }
    }

    fn delete(&mut self, block: &Block) {
        self.deleted[self.target].extend(block.line.no..=last(block));
    }

    fn prune(&mut self, tops: &[Vec<Block>]) {
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
        for (module, top) in tops.iter().enumerate() {
            for block in top {
                emptied(block, &mut self.deleted[module]);
            }
        }
    }

    fn append_rule(&mut self, section: &Block, name: &str) -> Result<(), Error> {
        let Some((head, conditions)) = section.children.split_first() else {
            return Err(Error::at(&section.line, "a rule edit needs a head sentence"));
        };
        self.appended[self.target].push((format!("rule {name}: {}", head.line.text.trim()), section.line.no));
        for condition in conditions {
            self.append_block(condition, condition.line.indent - 2);
        }
        Ok(())
    }

    fn append_children(&mut self, section: &Block, header: String) {
        self.appended[self.target].push((header, section.line.no));
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
            self.appended[self.target].push((format!("{prefix}{}", &raw[cut..]), no));
        }
    }

    fn place(&mut self, block: &Block, state: &BTreeSet<usize>) {
        let dedent = block.line.indent;
        let stated = |b: &Block| (b.line.no..=last(b)).any(|no| state.contains(&no));
        let table = block.line.tokens.iter().any(|t| matches!(t.kind, TokenKind::Variable(_)));
        if block.children.is_empty() || table {
            if stated(block) {
                self.appended[self.target].push(("initially".into(), block.line.no));
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
            self.appended[self.target].push(("initially".into(), block.line.no));
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
