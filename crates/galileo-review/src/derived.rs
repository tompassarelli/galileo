use crate::baseline::{definition_key, rule_key};
use galileo_core::why::sentence;
use galileo_core::{FactOrigin, Program, RelationId, RelationKind, SourceSpan, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivedEdit {
    pub sentence: String,
    pub at: Option<String>,
}

impl fmt::Display for DerivedEdit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.at {
            Some(at) => write!(f, "{}    {at}", self.sentence),
            None => write!(f, "{}", self.sentence),
        }
    }
}

fn kind(kind: &RelationKind) -> &'static str {
    match kind {
        RelationKind::Static => "given",
        RelationKind::State => "state",
        RelationKind::Derived => "derived",
        RelationKind::Event => "event",
        RelationKind::Effect => "effect",
    }
}

fn at(span: &Option<SourceSpan>) -> Option<String> {
    span.as_ref().map(|span| format!("{}:{}", span.file, span.line))
}

pub(crate) fn shared(program: &Program, joint: &BTreeMap<&str, usize>) -> BTreeMap<RelationId, RelationId> {
    program.relations.iter().enumerate().map(|(index, relation)| (RelationId(index), RelationId(joint[relation.name.as_str()]))).collect()
}

fn pair<K: Ord + Clone, V>(
    before: &BTreeMap<K, V>,
    after: &BTreeMap<K, V>,
    edits: &mut Vec<DerivedEdit>,
    mut removed: impl FnMut(&V) -> DerivedEdit,
    mut added: impl FnMut(&V) -> DerivedEdit,
    mut changed: impl FnMut(&V, &V) -> Option<DerivedEdit>,
) {
    for (key, old) in before {
        match after.get(key) {
            None => edits.push(removed(old)),
            Some(new) => edits.extend(changed(old, new)),
        }
    }
    for (key, new) in after {
        if !before.contains_key(key) {
            edits.push(added(new));
        }
    }
}

pub fn derived_edits(before: &Program, after: &Program) -> Vec<DerivedEdit> {
    let names: BTreeSet<&str> = before.relations.iter().chain(&after.relations).map(|r| r.name.as_str()).collect();
    let joint: BTreeMap<&str, usize> = names.into_iter().enumerate().map(|(index, name)| (name, index)).collect();
    let (before_ids, after_ids) = (shared(before, &joint), shared(after, &joint));
    let mut edits = Vec::new();

    let shapes = |program: &Program| program.relations.iter().map(|r| (r.name.clone(), r.clone())).collect::<BTreeMap<_, _>>();
    pair(
        &shapes(before),
        &shapes(after),
        &mut edits,
        |r| DerivedEdit { sentence: format!("removes the {} shape `{}`", kind(&r.kind), r.template), at: at(&r.source) },
        |r| DerivedEdit { sentence: format!("declares the {} shape `{}`", kind(&r.kind), r.template), at: at(&r.source) },
        |old, new| {
            if old.kind != new.kind {
                Some(DerivedEdit {
                    sentence: format!("changes the shape `{}` from {} to {}", new.template, kind(&old.kind), kind(&new.kind)),
                    at: at(&new.source),
                })
            } else {
                let strip = |r: &galileo_core::Relation| {
                    let mut r = r.clone();
                    r.source = None;
                    format!("{r:?}")
                };
                (strip(old) != strip(new)).then(|| DerivedEdit { sentence: format!("replaces the shape `{}`", new.template), at: at(&new.source) })
            }
        },
    );

    let entities = |program: &Program| program.entities.iter().map(|e| (e.name.clone(), e.clone())).collect::<BTreeMap<_, _>>();
    pair(
        &entities(before),
        &entities(after),
        &mut edits,
        |e| DerivedEdit { sentence: format!("removes the entity {} ({})", e.name, e.entity_type), at: None },
        |e| DerivedEdit { sentence: format!("declares the entity {} as a {}", e.name, e.entity_type), at: None },
        |old, new| (old.entity_type != new.entity_type).then(|| DerivedEdit {
            sentence: format!("redeclares the entity {} from {} to {}", new.name, old.entity_type, new.entity_type),
            at: None,
        }),
    );

    let facts = |program: &Program| -> BTreeMap<(String, Vec<Value>), DerivedEdit> {
        program
            .facts
            .iter()
            .map(|fact| {
                let location = match &fact.origin {
                    FactOrigin::Stated(span) => at(span),
                    FactOrigin::Foreign { source, revision, .. } => Some(format!("from {source}, extracted at {revision}")),
                };
                let text = sentence(program, fact.relation, &fact.values);
                ((program.relations[fact.relation.0].name.clone(), fact.values.clone()), DerivedEdit { sentence: text, at: location })
            })
            .collect()
    };
    pair(
        &facts(before),
        &facts(after),
        &mut edits,
        |e| DerivedEdit { sentence: format!("removes the fact `{}`", e.sentence), at: e.at.clone() },
        |e| DerivedEdit { sentence: format!("adds the fact `{}`", e.sentence), at: e.at.clone() },
        |_, _| None,
    );

    let rules = |program: &Program, ids: &BTreeMap<RelationId, RelationId>| {
        let mut grouped: BTreeMap<String, (Vec<String>, Option<String>)> = BTreeMap::new();
        for rule in &program.rules {
            let entry = grouped.entry(rule.name.clone()).or_default();
            entry.0.push(rule_key(rule, ids));
            entry.1.get_or_insert_with(|| at(&rule.source).unwrap_or_default());
        }
        grouped.into_iter().map(|(name, (mut keys, location))| {
            keys.sort();
            (name.clone(), (name, keys, location.filter(|l| !l.is_empty())))
        }).collect::<BTreeMap<_, _>>()
    };
    pair(
        &rules(before, &before_ids),
        &rules(after, &after_ids),
        &mut edits,
        |(name, _, location)| DerivedEdit { sentence: format!("removes rule `{name}`"), at: location.clone() },
        |(name, _, location)| DerivedEdit { sentence: format!("adds rule `{name}`"), at: location.clone() },
        |old, new| (old.1 != new.1).then(|| DerivedEdit { sentence: format!("replaces rule `{}`", new.0), at: new.2.clone() }),
    );

    let definitions = |program: &Program, ids: &BTreeMap<RelationId, RelationId>| {
        program.definitions.iter().map(|d| (d.name.clone(), (d.name.clone(), definition_key(d, ids), at(&d.source)))).collect::<BTreeMap<_, _>>()
    };
    pair(
        &definitions(before, &before_ids),
        &definitions(after, &after_ids),
        &mut edits,
        |(name, _, location)| DerivedEdit { sentence: format!("removes definition `{name}`"), at: location.clone() },
        |(name, _, location)| DerivedEdit { sentence: format!("adds definition `{name}`"), at: location.clone() },
        |old, new| (old.1 != new.1).then(|| DerivedEdit { sentence: format!("replaces definition `{}`", new.0), at: new.2.clone() }),
    );
    edits
}
