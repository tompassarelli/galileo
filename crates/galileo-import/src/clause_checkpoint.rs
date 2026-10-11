use crate::clause_term::{self, Identity, Referent};
use galileo_core::state::State;
use galileo_core::{Entity, Program, RelationKind, Type, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq)]
pub struct Converted {
    pub state: State,
    pub entities: Vec<(Referent, Entity)>,
    pub dropped: BTreeMap<String, usize>,
}

fn kebab(ty: &str) -> String {
    let mut out = String::new();
    for (at, c) in ty.chars().enumerate() {
        if at > 0 && c.is_ascii_uppercase() {
            out.push('-');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

pub fn table_name(term: &clause_term::Term, relation: &str) -> String {
    match relation.rsplit_once('.') {
        Some((ty, field)) => {
            let qualified = format!("{}-{field}", kebab(ty));
            if term.tables.contains_key(&qualified) { qualified } else { field.to_owned() }
        }
        None => relation.to_owned(),
    }
}

enum Slot {
    Plain(Value),
    Created(Referent, String),
}

pub fn convert(program: &Program, term: &[u8]) -> Result<Converted, String> {
    let term = clause_term::decode(term)?;
    let memberships: BTreeSet<String> = program.entity_types.iter().map(|ty| format!("is-a-{}", ty.name)).collect();
    let mut used = BTreeSet::new();
    let mut rows = Vec::new();
    let mut domains: BTreeMap<u32, String> = BTreeMap::new();
    for (at, relation) in program.relations.iter().enumerate() {
        if relation.kind != RelationKind::State || memberships.contains(&relation.name) {
            continue;
        }
        let name = &relation.name;
        if relation.slots.len() != 2 {
            return Err(format!("state relation {name} has {} slots; only binary relations convert", relation.slots.len()));
        }
        let table = table_name(&term, name);
        let Some(entries) = term.tables.get(&table) else {
            return Err(format!("state relation {name} has no table {table} in the Clause term"));
        };
        used.insert(table);
        for (subject, value) in entries {
            let mut slots = Vec::new();
            for (slot, value) in relation.slots.iter().zip([clause_term::Value::Referent(*subject), value.clone()]) {
                let converted = match (&slot.ty, value) {
                    (Type::Text, clause_term::Value::Text(text)) => Slot::Plain(Value::Text(text)),
                    (Type::Bool, clause_term::Value::Bool(value)) => Slot::Plain(Value::Bool(value)),
                    (Type::Int, clause_term::Value::Number(number)) => {
                        if number.fract() != 0.0 || number.abs() > (1u64 << 53) as f64 {
                            return Err(format!("state relation {name} holds {number}, not an integer of magnitude at most 2^53"));
                        }
                        Slot::Plain(Value::Int(number as i64))
                    }
                    (Type::Entity(ty), clause_term::Value::Referent(referent)) => match referent.identity {
                        Identity::Declared(_) => {
                            let singleton = term
                                .singletons
                                .iter()
                                .find(|(_, other)| **other == referent)
                                .map(|(singleton, _)| singleton)
                                .ok_or_else(|| format!("state relation {name} holds declared referent {referent:?}, which no singleton names"))?;
                            let entity = program
                                .entities
                                .iter()
                                .find(|entity| entity.name == *singleton)
                                .ok_or_else(|| format!("state relation {name} holds singleton {singleton}, which the program does not declare"))?;
                            if entity.entity_type != *ty {
                                return Err(format!("state relation {name} holds {singleton}, a {}, in a {ty} slot", entity.entity_type));
                            }
                            Slot::Plain(Value::Entity(entity.clone()))
                        }
                        Identity::Created(_) => {
                            if let Some(previous) = domains.insert(referent.domain, ty.clone()) {
                                if previous != *ty {
                                    return Err(format!("state relation {name} gives Clause domain {} the type {ty}, which already has type {previous}", referent.domain));
                                }
                            }
                            Slot::Created(referent, ty.clone())
                        }
                    },
                    (Type::Text | Type::Bool | Type::Int | Type::Entity(_), value) => {
                        return Err(format!("state relation {name} holds {value:?} in a {:?} slot", slot.ty));
                    }
                    (ty, value) => {
                        return Err(format!("state relation {name} holds {value:?} in a {ty:?} slot, which the importer does not read"));
                    }
                };
                slots.push(converted);
            }
            rows.push((galileo_core::RelationId(at), slots));
        }
    }
    let mut created: Vec<(String, [u8; 32], Referent)> = Vec::new();
    for (_, slots) in &rows {
        for slot in slots {
            if let Slot::Created(referent, ty) = slot {
                let Identity::Created(bytes) = referent.identity else { unreachable!() };
                created.push((ty.clone(), bytes, *referent));
            }
        }
    }
    created.sort();
    created.dedup();
    let entities: Vec<(Referent, Entity)> = created
        .into_iter()
        .enumerate()
        .map(|(id, (ty, _, referent))| (referent, Entity::created(ty.clone(), format!("{ty}#{id}"), id as u64)))
        .collect();
    let lookup: BTreeMap<Referent, &Entity> = entities.iter().map(|(referent, entity)| (*referent, entity)).collect();
    let mut state = State { next: entities.len() as u64, ..State::default() };
    for (_, entity) in &entities {
        state.live.insert(entity.clone());
        if let Some(membership) = program.relation_id(&format!("is-a-{}", entity.entity_type)) {
            state.facts.insert((membership, vec![Value::Entity(entity.clone())]));
        }
    }
    for (relation, slots) in rows {
        let values = slots
            .into_iter()
            .map(|slot| match slot {
                Slot::Plain(value) => value,
                Slot::Created(referent, _) => Value::Entity(lookup[&referent].clone()),
            })
            .collect();
        state.facts.insert((relation, values));
    }
    let dropped = term
        .tables
        .iter()
        .filter(|(name, _)| !used.contains(*name))
        .map(|(name, rows)| (name.clone(), rows.len()))
        .collect();
    Ok(Converted { state, entities, dropped })
}
