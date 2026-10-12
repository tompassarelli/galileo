use crate::{Error, Program, State, Tuple, Value};
use galileo_core::{Entity, RelationId};
use std::collections::{BTreeMap, BTreeSet};

const LARGE_TEXT: usize = 4096;

struct Writer {
    parts: Vec<Vec<u8>>,
    current: Vec<u8>,
}

impl Writer {
    fn number(&mut self, number: u64) {
        self.current.extend_from_slice(&number.to_le_bytes());
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.number(bytes.len() as u64);
        self.current.extend_from_slice(bytes);
    }

    fn text(&mut self, text: &str) {
        if text.len() < LARGE_TEXT {
            return self.bytes(text.as_bytes());
        }
        self.number(text.len() as u64);
        self.parts.push(std::mem::take(&mut self.current));
        self.parts.push(text.as_bytes().to_vec());
    }

    fn tuple(&mut self, program: &Program, (relation, values): &Tuple) {
        self.bytes(program.relations[relation.0].name.as_bytes());
        self.values(values);
    }

    fn values(&mut self, values: &[Value]) {
        self.number(values.len() as u64);
        for value in values {
            self.value(value);
        }
    }

    fn entity(&mut self, entity: &Entity) {
        self.bytes(entity.name.as_bytes());
        match entity.creation_order {
            None => self.current.push(0),
            Some(order) => {
                self.current.push(1);
                self.number(order);
            }
        }
        self.bytes(entity.entity_type.as_bytes());
    }

    fn map(&mut self, map: &BTreeMap<String, Value>) {
        self.number(map.len() as u64);
        for (key, value) in map {
            self.bytes(key.as_bytes());
            self.value(value);
        }
    }

    fn value(&mut self, value: &Value) {
        match value {
            Value::Null => self.current.push(0),
            Value::Bool(flag) => {
                self.current.push(1);
                self.current.push(u8::from(*flag));
            }
            Value::Int(number) => {
                self.current.push(2);
                self.current.extend_from_slice(&number.to_le_bytes());
            }
            Value::Text(text) => {
                self.current.push(3);
                self.text(text);
            }
            Value::Path(path) => {
                self.current.push(4);
                self.bytes(path.as_bytes());
            }
            Value::OptionPath(path) => {
                self.current.push(5);
                self.number(path.len() as u64);
                for segment in path {
                    self.bytes(segment.as_bytes());
                }
            }
            Value::Entity(entity) => {
                self.current.push(6);
                self.entity(entity);
            }
            Value::List(items) => {
                self.current.push(7);
                self.values(items);
            }
            Value::Record(fields) => {
                self.current.push(8);
                self.map(fields);
            }
            Value::Attrs(fields) => {
                self.current.push(9);
                self.map(fields);
            }
            Value::Nix { normalized } => {
                self.current.push(10);
                self.bytes(normalized.as_bytes());
            }
        }
    }
}

pub(crate) fn save(program: &Program, state: &State, revision: u64) -> Vec<Vec<u8>> {
    let mut writer = Writer {
        parts: Vec::new(),
        current: format!("galileo-save 1 {:016x} {revision}\n", program.identity.0).into_bytes(),
    };
    writer.number(state.next);
    writer.number(state.facts.len() as u64);
    for fact in &state.facts {
        writer.tuple(program, fact);
    }
    writer.number(state.live.len() as u64);
    for entity in &state.live {
        writer.entity(entity);
    }
    writer.number(state.pending.len() as u64);
    for request in &state.pending {
        writer.tuple(program, request);
    }
    let mut parts = writer.parts;
    if !writer.current.is_empty() {
        parts.push(writer.current);
    }
    parts
}

struct Reader<'a> {
    program: &'a Program,
    relations: BTreeMap<&'a str, RelationId>,
    remaining: &'a [u8],
}

fn saved(message: impl Into<String>) -> Error {
    Error::Saved(message.into())
}

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize, what: &str) -> Result<&'a [u8], Error> {
        if self.remaining.len() < length {
            return Err(saved(format!("truncated save: {what} needs {length} bytes, {} remain", self.remaining.len())));
        }
        let (taken, rest) = self.remaining.split_at(length);
        self.remaining = rest;
        Ok(taken)
    }

    fn byte(&mut self, what: &str) -> Result<u8, Error> {
        Ok(self.take(1, what)?[0])
    }

    fn number(&mut self, what: &str) -> Result<u64, Error> {
        let bytes = self.take(8, what)?;
        Ok(u64::from_le_bytes(bytes.try_into().expect("eight bytes")))
    }

    fn count(&mut self, what: &str) -> Result<usize, Error> {
        let count = self.number(what)?;
        usize::try_from(count).map_err(|_| saved(format!("{what} count {count} is too large")))
    }

    fn string(&mut self, what: &str) -> Result<String, Error> {
        let length = self.count(what)?;
        let bytes = self.take(length, what)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| saved(format!("{what} is not UTF-8")))
    }

    fn tuple(&mut self, what: &str) -> Result<Tuple, Error> {
        let name = self.string(what)?;
        let relation = *self
            .relations
            .get(name.as_str())
            .ok_or_else(|| saved(format!("{what} names relation {name:?}, which the program does not declare")))?;
        let arity = self.program.relations[relation.0].slots.len();
        let values = self.values(what)?;
        if values.len() != arity {
            return Err(saved(format!("{what} of {name:?} has {} values, the relation takes {arity}", values.len())));
        }
        Ok((relation, values))
    }

    fn values(&mut self, what: &str) -> Result<Vec<Value>, Error> {
        let count = self.count(what)?;
        let mut values = Vec::new();
        for _ in 0..count {
            values.push(self.value(what)?);
        }
        Ok(values)
    }

    fn entity(&mut self, what: &str) -> Result<Entity, Error> {
        let name = self.string(what)?;
        let creation_order = match self.byte(what)? {
            0 => None,
            1 => Some(self.number(what)?),
            tag => return Err(saved(format!("{what} has unknown creation order tag {tag}"))),
        };
        let entity_type = self.string(what)?;
        Ok(Entity { name, creation_order, entity_type })
    }

    fn map(&mut self, what: &str) -> Result<BTreeMap<String, Value>, Error> {
        let count = self.count(what)?;
        let mut map = BTreeMap::new();
        for _ in 0..count {
            let key = self.string(what)?;
            let value = self.value(what)?;
            map.insert(key, value);
        }
        Ok(map)
    }

    fn value(&mut self, what: &str) -> Result<Value, Error> {
        Ok(match self.byte(what)? {
            0 => Value::Null,
            1 => match self.byte(what)? {
                0 => Value::Bool(false),
                1 => Value::Bool(true),
                flag => return Err(saved(format!("{what} has Bool byte {flag}"))),
            },
            2 => Value::Int(i64::from_le_bytes(self.take(8, what)?.try_into().expect("eight bytes"))),
            3 => Value::Text(self.string(what)?),
            4 => Value::Path(self.string(what)?),
            5 => {
                let count = self.count(what)?;
                let mut path = Vec::new();
                for _ in 0..count {
                    path.push(self.string(what)?);
                }
                Value::OptionPath(path)
            }
            6 => Value::Entity(self.entity(what)?),
            7 => Value::List(self.values(what)?),
            8 => Value::Record(self.map(what)?),
            9 => Value::Attrs(self.map(what)?),
            10 => Value::Nix { normalized: self.string(what)? },
            tag => return Err(saved(format!("{what} has unknown value tag {tag}"))),
        })
    }
}

pub(crate) fn open(program: &Program, bytes: &[u8]) -> Result<(State, u64), Error> {
    let newline = bytes
        .iter()
        .position(|&byte| byte == b'\n')
        .ok_or_else(|| saved("expected a header line galileo-save 1 <identity> <revision>"))?;
    let header = std::str::from_utf8(&bytes[..newline])
        .map_err(|_| saved("the header line is not UTF-8"))?;
    let (identity, revision) = header
        .strip_prefix("galileo-save 1 ")
        .and_then(|rest| rest.split_once(' '))
        .ok_or_else(|| saved(format!("expected galileo-save 1 <identity> <revision>, found {header:?}")))?;
    if identity.len() != 16 || !identity.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')) {
        return Err(saved(format!("expected a 16 lowercase hex identity, found {identity:?}")));
    }
    let identity = u64::from_str_radix(identity, 16).expect("checked hex");
    if !revision.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(saved(format!("expected a decimal revision, found {revision:?}")));
    }
    let revision = revision
        .parse::<u64>()
        .map_err(|_| saved(format!("expected a decimal revision, found {revision:?}")))?;
    if identity != program.identity.0 {
        return Err(Error::WrongProgram);
    }
    let mut reader = Reader {
        program,
        relations: program
            .relations
            .iter()
            .enumerate()
            .rev()
            .map(|(index, relation)| (relation.name.as_str(), RelationId(index)))
            .collect(),
        remaining: &bytes[newline + 1..],
    };
    let next = reader.number("the allocator")?;
    let mut facts = BTreeSet::new();
    for _ in 0..reader.count("the facts")? {
        facts.insert(reader.tuple("a fact")?);
    }
    let mut live = BTreeSet::new();
    for _ in 0..reader.count("the live entities")? {
        live.insert(reader.entity("a live entity")?);
    }
    let mut pending = Vec::new();
    for _ in 0..reader.count("the pending requests")? {
        pending.push(reader.tuple("a pending request")?);
    }
    if !reader.remaining.is_empty() {
        return Err(saved(format!("{} bytes follow the pending requests", reader.remaining.len())));
    }
    Ok((State { facts, live, next, pending }, revision))
}
