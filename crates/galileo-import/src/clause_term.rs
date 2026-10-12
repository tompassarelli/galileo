use std::collections::BTreeMap;

const FIELD: &[u8] = b"clause/js-field-v1";
const OBJECT_END: &[u8] = b"clause/js-object-end-v1";
const REFERENT: &[u8] = b"clause/process-projected-referent-v1";
const TABLE: &[u8] = b"clause/process-projected-relation-table-v1";
const TEXT: &[u8] = b"clause/process-projected-text-v1";
const F64: &[u8] = b"clause/process-projected-f64-v1";
const BOOL: &[u8] = b"clause/process-projected-bool-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Identity {
    Declared(u32),
    Created([u8; 32]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Referent {
    pub domain: u32,
    pub identity: Identity,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f64),
    Bool(bool),
    Text(String),
    Referent(Referent),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Term {
    pub tables: BTreeMap<String, Vec<(Referent, Value)>>,
    pub singletons: BTreeMap<String, Referent>,
    pub fields: BTreeMap<String, Vec<(Referent, Value)>>,
}

enum Node<'a> {
    Atom { kind: &'a [u8], payload: &'a [u8] },
    Triple(Box<[Node<'a>; 3]>),
}

struct Reader<'a, 'b> {
    input: &'a [u8],
    context: &'b str,
}

impl<'a> Reader<'a, '_> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        if self.input.len() < count {
            return Err(format!(
                "truncated Clause term in {}: needed {count} bytes, {} left",
                self.context,
                self.input.len()
            ));
        }
        let (head, tail) = self.input.split_at(count);
        self.input = tail;
        Ok(head)
    }

    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u16_le(&mut self) -> Result<usize, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()) as usize)
    }

    fn u32_le(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u32_be(&mut self) -> Result<usize, String> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()) as usize)
    }

    fn node(&mut self) -> Result<Node<'a>, String> {
        match self.byte()? {
            0 => {
                let length = self.u32_be()?;
                let kind = self.take(length)?;
                let length = self.u32_be()?;
                let payload = self.take(length)?;
                self.byte()?;
                Ok(Node::Atom { kind, payload })
            }
            1 => Ok(Node::Triple(Box::new([self.node()?, self.node()?, self.node()?]))),
            tag => Err(format!("unknown Clause term node tag {tag} in {}", self.context)),
        }
    }

    fn referent(&mut self) -> Result<Referent, String> {
        let domain = self.u32_le()?;
        let identity = match self.byte()? {
            0 => Identity::Declared(self.u32_le()?),
            1 => Identity::Created(self.take(32)?.try_into().unwrap()),
            tag => {
                return Err(format!(
                    "unknown Clause referent identity tag {tag} in {}",
                    self.context
                ))
            }
        };
        Ok(Referent { domain, identity })
    }

    fn value(&mut self) -> Result<Value, String> {
        Ok(match self.byte()? {
            0 => Value::Number(f64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            1 => Value::Bool(self.byte()? == 1),
            4 => {
                let length = self.u16_le()?;
                Value::Text(self.text(length)?)
            }
            7 => {
                let length = self.u32_le()? as usize;
                Value::Text(self.text(length)?)
            }
            5 => Value::Referent(self.referent()?),
            tag => return Err(format!("unknown Clause value tag {tag} in {}", self.context)),
        })
    }

    fn text(&mut self, length: usize) -> Result<String, String> {
        let bytes = self.take(length)?;
        String::from_utf8(bytes.to_vec())
            .map_err(|_| format!("Clause text in {} is not UTF-8", self.context))
    }

    fn finish(&self) -> Result<(), String> {
        if self.input.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "{} trailing bytes after Clause {}",
                self.input.len(),
                self.context
            ))
        }
    }
}

fn object<'n, 'a>(mut node: &'n Node<'a>, context: &str) -> Result<Vec<(&'a str, &'n Node<'a>)>, String> {
    let mut fields = Vec::new();
    loop {
        match node {
            Node::Atom { kind, .. } if *kind == OBJECT_END => return Ok(fields),
            Node::Triple(slots) => {
                let [Node::Atom { kind, payload }, value, rest] = &**slots else {
                    return Err(format!("Clause object {context} has a non-atom field key"));
                };
                if *kind != FIELD {
                    return Err(format!("Clause object {context} has a key that is not a field"));
                }
                let name = std::str::from_utf8(payload)
                    .map_err(|_| format!("Clause object {context} has a non-UTF-8 field name"))?;
                fields.push((name, value));
                node = rest;
            }
            Node::Atom { .. } => return Err(format!("Clause object {context} is not an object")),
        }
    }
}

fn table(name: &str, payload: &[u8]) -> Result<Vec<(Referent, Value)>, String> {
    let context = format!("table {name}");
    let mut input = Reader { input: payload, context: &context };
    let tag = input.byte()?;
    if tag != 6 {
        return Err(format!("unknown Clause value tag {tag} in {context}, expected a relation table"));
    }
    input.take(5)?;
    if input.byte()? == 1 {
        input.take(4)?;
    }
    input.byte()?;
    let mut rows = Vec::new();
    for _ in 0..input.u16_le()? {
        let subject = input.referent()?;
        for _ in 0..input.u16_le()? {
            rows.push((subject, input.value()?));
        }
    }
    input.finish()?;
    Ok(rows)
}

fn field_value(singleton: &str, field: &str, node: &Node) -> Result<Option<Value>, String> {
    let Node::Atom { kind, payload } = node else { return Ok(None) };
    let context = format!("field {field} of singleton {singleton}");
    Ok(Some(match *kind {
        TEXT => Value::Text(String::from_utf8(payload.to_vec()).map_err(|_| format!("{context} is not UTF-8"))?),
        F64 => Value::Number(f64::from_le_bytes((*payload).try_into().map_err(|_| format!("{context} is not 8 bytes"))?)),
        BOOL => match payload {
            [0] => Value::Bool(false),
            [1] => Value::Bool(true),
            _ => return Err(format!("{context} is not a Clause bool")),
        },
        REFERENT => {
            let mut reader = Reader { input: payload, context: &context };
            let referent = reader.referent()?;
            reader.finish()?;
            Value::Referent(referent)
        }
        _ => return Ok(None),
    }))
}

pub fn decode(bytes: &[u8]) -> Result<Term, String> {
    let mut input = Reader { input: bytes, context: "term" };
    input.take(64)?;
    let root = input.node()?;
    input.finish()?;
    let mut term = Term::default();
    let mut singleton_fields = Vec::new();
    for (name, value) in object(&root, "term")? {
        if name == "relations" {
            for (table_name, value) in object(value, "relations")? {
                let Node::Atom { kind, payload } = value else {
                    return Err(format!("table {table_name} is not an atom"));
                };
                if *kind != TABLE {
                    return Err(format!("table {table_name} has kind {}", String::from_utf8_lossy(kind)));
                }
                if term.tables.insert(table_name.to_owned(), table(table_name, payload)?).is_some() {
                    return Err(format!("duplicate table {table_name}"));
                }
            }
            continue;
        }
        let Node::Triple(_) = value else { continue };
        let fields = object(value, name)?;
        let Some((_, Node::Atom { kind, payload })) = fields.iter().find(|(field, _)| *field == "$referent") else {
            continue;
        };
        if *kind != REFERENT {
            return Err(format!("singleton {name} has a $referent of kind {}", String::from_utf8_lossy(kind)));
        }
        let context = format!("singleton {name}");
        let mut reader = Reader { input: payload, context: &context };
        let referent = reader.referent()?;
        reader.finish()?;
        term.singletons.insert(name.to_owned(), referent);
        for (field, value) in fields {
            if field != "$referent" {
                singleton_fields.push((name, field, referent, value));
            }
        }
    }
    for (singleton, field, referent, value) in singleton_fields {
        if term.tables.contains_key(field) {
            continue;
        }
        if let Some(value) = field_value(singleton, field, value)? {
            term.fields.entry(field.to_owned()).or_default().push((referent, value));
        }
    }
    Ok(term)
}
