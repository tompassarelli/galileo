#![forbid(unsafe_code)]

use galileo_core::{BinaryOp, Expr, Literal, Mode, Program, QueryKind, RelationKind, Type, Value};
use galileo_syntax::Vocabulary;
use std::collections::{BTreeMap, BTreeSet};

pub fn rust(program: &Program, vocabularies: &[&Vocabulary]) -> Result<String, String> {
    let mut generator = Generator { program, unions: BTreeMap::new(), names: BTreeSet::new(), output: String::from(
        "use galileo_core::{Entity, EvalError, Value};\nuse galileo_runtime::{Program, State};\n\nfn invalid(at: &str) -> EvalError { EvalError::Type { at: at.into(), message: \"value does not match generated binding\".into() } }\n"
    ) };
    for vocabulary in vocabularies {
        for (name, members) in &vocabulary.types {
            if let Some(members) = members {
                generator
                    .unions
                    .entry(members.clone())
                    .or_insert_with(|| pascal(name));
            }
        }
    }
    for (members, name) in generator.unions.clone() {
        generator.reserve(&name)?;
        let variants = members
            .iter()
            .map(|member| Ok(format!("    {},\n", pascal(member))))
            .collect::<Result<String, String>>()?;
        unique(members.iter().map(|member| pascal(member)))?;
        generator.output.push_str(&format!("\n#[derive(Clone, Debug, PartialEq, Eq)]\npub enum {name} {{\n{variants}}}\nimpl {name} {{\n    pub fn into_value(self) -> Value {{ Value::Text(match self {{\n"));
        for member in &members {
            generator.output.push_str(&format!(
                "        Self::{} => {:?}.into(),\n",
                pascal(member),
                member
            ));
        }
        generator.output.push_str("    }) }\n    pub fn from_value(value: &Value) -> Result<Self, EvalError> { match value {\n");
        for member in &members {
            generator.output.push_str(&format!(
                "        Value::Text(text) if text == {:?} => Ok(Self::{}),\n",
                member,
                pascal(member)
            ));
        }
        generator.output.push_str(&format!(
            "        _ => Err(invalid({name:?})),\n    }} }}\n}}\n"
        ));
    }
    for entity in &program.entity_types {
        let name = pascal(&entity.name);
        generator.reserve(&name)?;
        let mut methods = String::new();
        unique(entity.fields.iter().map(|slot| snake(&slot.name)))?;
        for field in &entity.fields {
            let ty = generator.ty(&field.ty, &format!("{name}{}", pascal(&field.name)))?;
            let relation = program
                .field_relation(&entity.name, &field.name)
                .ok_or_else(|| format!("missing field {}.{}", entity.name, field.name))?;
            let decode = generator.decode(&field.ty, "value", &ty)?;
            methods.push_str(&format!("    pub fn {}(&self, state: &State, program: &Program) -> Result<{ty}, EvalError> {{\n        let value = state.facts.iter().find(|(relation, values)| Some(*relation) == program.relation_id({:?}) && values.first() == Some(&Value::Entity(self.0.clone()))).and_then(|(_, values)| values.get(1)).unwrap_or(&Value::Null);\n        {decode}\n    }}\n", snake(&field.name), program.relations[relation.0].name));
        }
        generator.output.push_str(&format!("\n#[derive(Clone, Debug, PartialEq, Eq)]\npub struct {name}(pub Entity);\nimpl {name} {{\n    pub fn named(name: impl Into<String>) -> Self {{ Self(Entity::named({:?}, name)) }}\n    pub fn into_value(self) -> Value {{ Value::Entity(self.0) }}\n    pub fn from_value(value: &Value) -> Result<Self, EvalError> {{ match value {{ Value::Entity(entity) if entity.entity_type == {:?} => Ok(Self(entity.clone())), _ => Err(invalid({name:?})) }} }}\n", entity.name, entity.name));
        generator.output.push_str(&methods);
        generator.output.push_str("}\n");
    }
    generator.events(RelationKind::Event, "Event")?;
    generator.events(RelationKind::Effect, "Effect")?;
    for view in &program.views {
        let name = pascal(&view.name);
        let variables = view
            .parameters
            .iter()
            .map(|slot| (slot.name.clone(), slot.ty.clone()))
            .collect();
        let fields = view
            .fields
            .iter()
            .map(|(field, expr)| Ok((field.clone(), generator.infer(expr, &variables)?)))
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        generator.record(&name, &fields, true)?;
        let parameters = view
            .parameters
            .iter()
            .map(|slot| {
                Ok(format!(
                    "{}: {}",
                    snake(&slot.name),
                    generator.ty(&slot.ty, &format!("{name}{}", pascal(&slot.name)))?
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let values = view
            .parameters
            .iter()
            .map(|slot| generator.encode(&slot.ty, &snake(&slot.name)))
            .collect::<Result<Vec<_>, _>>()?;
        let argument_types = parameters
            .iter()
            .map(|parameter| parameter.split_once(": ").unwrap().1)
            .collect::<Vec<_>>();
        let argument_names = view
            .parameters
            .iter()
            .map(|slot| snake(&slot.name))
            .collect::<Vec<_>>();
        let tuple = |items: &[String]| match items {
            [] => "()".into(),
            [one] => format!("({one},)"),
            many => format!("({})", many.join(", ")),
        };
        let types = argument_types
            .iter()
            .map(|ty| ty.to_string())
            .collect::<Vec<_>>();
        generator.output.push_str(&format!("impl galileo_runtime::Projection for {name} {{\n    type Arguments = {};\n    fn view(program: &Program) -> &galileo_core::View {{ program.views.iter().find(|view| view.name == {:?}).expect(\"generated view belongs to this program\") }}\n    fn arguments(arguments: Self::Arguments) -> Vec<Value> {{ let {} = arguments; vec![{}] }}\n    fn decode(fields: &mut galileo_runtime::Fields<'_>) -> Result<Self, EvalError> {{ Self::from_fields(fields) }}\n}}\n", tuple(&types), view.name, tuple(&argument_names), values.join(", ")));
    }
    Ok(generator.output)
}

struct Generator<'p> {
    program: &'p Program,
    unions: BTreeMap<Vec<String>, String>,
    names: BTreeSet<String>,
    output: String,
}

impl Generator<'_> {
    fn reserve(&mut self, name: &str) -> Result<(), String> {
        if self.names.insert(name.into()) {
            Ok(())
        } else {
            Err(format!("generated Rust name collision: {name}"))
        }
    }

    fn ty(&mut self, ty: &Type, context: &str) -> Result<String, String> {
        Ok(match ty {
            Type::Bool => "bool".into(),
            Type::Int => "i64".into(),
            Type::Text | Type::Path => "String".into(),
            Type::OptionPath => "Vec<String>".into(),
            Type::Entity(name) => pascal(name),
            Type::TextUnion(members) => self
                .unions
                .get(members)
                .cloned()
                .ok_or_else(|| format!("unnamed text union in {context}"))?,
            Type::Maybe(inner) => format!("Option<{}>", self.ty(inner, context)?),
            Type::List(inner) => format!("Vec<{}>", self.ty(inner, context)?),
            Type::Record(fields) => {
                self.record(context, fields, false)?;
                context.into()
            }
            Type::Attrs(inner) => format!(
                "std::collections::BTreeMap<String, {}>",
                self.ty(inner, context)?
            ),
            Type::Any => "Value".into(),
            _ => return Err(format!("unsupported Rust binding type {ty:?} in {context}")),
        })
    }

    fn encode(&self, ty: &Type, value: &str) -> Result<String, String> {
        Ok(match ty {
            Type::Bool => format!("Value::Bool({value})"),
            Type::Int => format!("Value::Int({value})"),
            Type::Text => format!("Value::Text({value})"),
            Type::Path => format!("Value::Path({value})"),
            Type::OptionPath => format!("Value::OptionPath({value})"),
            Type::Entity(_) | Type::TextUnion(_) | Type::Record(_) => {
                format!("{value}.into_value()")
            }
            Type::Maybe(inner) => format!(
                "{value}.map(|item| {}).unwrap_or(Value::Null)",
                self.encode(inner, "item")?
            ),
            Type::List(inner) => format!(
                "Value::List({value}.into_iter().map(|item| {}).collect())",
                self.encode(inner, "item")?
            ),
            Type::Attrs(inner) => format!(
                "Value::Attrs({value}.into_iter().map(|(key, item)| (key, {})).collect())",
                self.encode(inner, "item")?
            ),
            Type::Any => value.into(),
            _ => return Err(format!("unsupported encoding {ty:?}")),
        })
    }

    fn decode(&self, ty: &Type, value: &str, rust_ty: &str) -> Result<String, String> {
        let invalid = "Err(invalid(\"decode\"))";
        Ok(match ty {
            Type::Any => format!("Ok(({value}).clone())"),
            Type::Bool | Type::Int | Type::Text | Type::Path | Type::OptionPath => {
                let variant = match ty {
                    Type::Bool => "Bool",
                    Type::Int => "Int",
                    Type::Text => "Text",
                    Type::Path => "Path",
                    _ => "OptionPath",
                };
                format!("match {value} {{ Value::{variant}(item) => Ok(item.clone()), _ => {invalid} }}")
            }
            Type::Entity(_) | Type::TextUnion(_) | Type::Record(_) => {
                format!("{rust_ty}::from_value({value})")
            }
            Type::Maybe(inner) => {
                let inner_ty = rust_ty
                    .strip_prefix("Option<")
                    .and_then(|s| s.strip_suffix('>'))
                    .unwrap();
                format!(
                    "match {value} {{ Value::Null => Ok(None), item => ({}).map(Some) }}",
                    self.decode(inner, "item", inner_ty)?
                )
            }
            Type::List(inner) => {
                let shared = rust_ty.starts_with("std::sync::Arc<[");
                let inner_ty = if shared {
                    rust_ty
                        .strip_prefix("std::sync::Arc<[")
                        .and_then(|s| s.strip_suffix("]>"))
                } else {
                    rust_ty
                        .strip_prefix("Vec<")
                        .and_then(|s| s.strip_suffix('>'))
                }
                .unwrap();
                let convert = if shared {
                    ".map(std::sync::Arc::from)"
                } else {
                    ""
                };
                format!("match {value} {{ Value::List(items) => items.iter().map(|item| {}).collect::<Result<Vec<_>, EvalError>>(){convert}, _ => {invalid} }}", self.decode(inner, "item", inner_ty)?)
            }
            Type::Attrs(inner) => {
                let inner_ty = rust_ty
                    .strip_prefix("std::collections::BTreeMap<String, ")
                    .and_then(|s| s.strip_suffix('>'))
                    .unwrap();
                format!("match {value} {{ Value::Attrs(items) => items.iter().map(|(key, item)| ({}).map(|item| (key.clone(), item))).collect::<Result<_, EvalError>>(), _ => {invalid} }}", self.decode(inner, "item", inner_ty)?)
            }
            _ => return Err(format!("unsupported decoding {ty:?}")),
        })
    }

    fn record(
        &mut self,
        name: &str,
        fields: &BTreeMap<String, Type>,
        shared: bool,
    ) -> Result<(), String> {
        self.reserve(name)?;
        unique(fields.keys().map(|name| snake(name)))?;
        let fields = fields
            .iter()
            .map(|(field, ty)| {
                let rust_ty = if shared {
                    if let Type::List(inner) = ty {
                        format!(
                            "std::sync::Arc<[{}]>",
                            self.ty(inner, &format!("{name}{}", pascal(field)))?
                        )
                    } else {
                        self.ty(ty, &format!("{name}{}", pascal(field)))?
                    }
                } else {
                    self.ty(ty, &format!("{name}{}", pascal(field)))?
                };
                Ok((field, ty, rust_ty))
            })
            .collect::<Result<Vec<_>, String>>()?;
        self.output.push_str(&format!(
            "\n#[derive(Clone, Debug, PartialEq, Eq)]\npub struct {name} {{\n"
        ));
        for (field, _, ty) in &fields {
            self.output
                .push_str(&format!("    pub {}: {ty},\n", snake(field)));
        }
        self.output.push_str(&format!("}}\nimpl {name} {{\n    pub fn from_value(value: &Value) -> Result<Self, EvalError> {{\n        let Value::Record(fields) = value else {{ return Err(invalid({name:?})); }};\n        let mut fields = fields.iter();\n        Ok(Self {{\n"));
        for (field, ty, rust_ty) in &fields {
            self.output.push_str(&format!(
                "            {}: ({})?,\n",
                snake(field),
                self.decode(
                    ty,
                    &format!("fields.find(|(name, _)| name.as_str() == {field:?}).map(|(_, value)| value).ok_or_else(|| invalid({field:?}))?"),
                    rust_ty
                )?
            ));
        }
        self.output.push_str("        })\n    }\n");
        if shared {
            self.output.push_str("    fn from_fields(fields: &mut galileo_runtime::Fields<'_>) -> Result<Self, EvalError> {\n        Ok(Self {\n");
            for (field, ty, rust_ty) in &fields {
                self.output.push_str(&format!(
                    "            {}: fields.field::<{rust_ty}>({field:?}, |value| {})?,\n",
                    snake(field),
                    self.decode(ty, "value", rust_ty)?
                ));
            }
            self.output.push_str("        })\n    }\n");
        }
        self.output.push_str("    pub fn into_value(self) -> Value { Value::Record(std::collections::BTreeMap::from([\n");
        for (field, ty, _) in &fields {
            self.output.push_str(&format!(
                "        ({field:?}.into(), {}),\n",
                self.encode(
                    ty,
                    &if shared && matches!(ty, Type::List(_)) {
                        format!("self.{}.iter().cloned()", snake(field))
                    } else {
                        format!("self.{}", snake(field))
                    }
                )?
            ));
        }
        self.output.push_str("    ])) }\n}\n");
        Ok(())
    }

    fn events(&mut self, kind: RelationKind, name: &str) -> Result<(), String> {
        self.reserve(name)?;
        let relations = self
            .program
            .relations
            .iter()
            .enumerate()
            .filter(|(_, relation)| relation.kind == kind)
            .collect::<Vec<_>>();
        unique(relations.iter().map(|(_, relation)| pascal(&relation.name)))?;
        let mut variants = Vec::new();
        for (_, relation) in relations {
            let variant = pascal(&relation.name);
            unique(relation.slots.iter().map(|slot| snake(&slot.name)))?;
            let fields = relation
                .slots
                .iter()
                .map(|slot| {
                    Ok((
                        slot,
                        self.ty(&slot.ty, &format!("{variant}{}", pascal(&slot.name)))?,
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?;
            variants.push((relation.name.clone(), variant, fields));
        }
        if kind == RelationKind::Event {
            for (_, variant, fields) in &variants {
                self.reserve(variant)?;
                self.output.push_str(&format!(
                    "\n#[derive(Clone, Debug, PartialEq, Eq)]\npub struct {variant} {{\n"
                ));
                for (slot, ty) in fields {
                    self.output
                        .push_str(&format!("    pub {}: {ty},\n", snake(&slot.name)));
                }
                let values = fields
                    .iter()
                    .map(|(slot, _)| {
                        format!("{}: self.{}.clone()", snake(&slot.name), snake(&slot.name))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                self.output.push_str(&format!("}}\nimpl galileo_runtime::Event for {variant} {{\n    fn tuple(&self, program: &Program) -> galileo_runtime::Tuple {{ galileo_runtime::Event::tuple(&Event::{variant} {{ {values} }}, program) }}\n}}\n"));
            }
        }
        self.output.push_str(&format!(
            "\n#[derive(Clone, Debug, PartialEq, Eq)]\npub enum {name} {{\n"
        ));
        for (_, variant, fields) in &variants {
            self.output.push_str(&format!(
                "    {variant} {{ {} }},\n",
                fields
                    .iter()
                    .map(|(slot, ty)| format!("{}: {ty}", snake(&slot.name)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        self.output.push_str(&format!("}}\nimpl {name} {{\n    pub fn into_tuple(self, program: &Program) -> galileo_runtime::Tuple {{ match self {{\n"));
        for (canonical, variant, fields) in &variants {
            let names = fields
                .iter()
                .map(|(slot, _)| snake(&slot.name))
                .collect::<Vec<_>>();
            let values = fields
                .iter()
                .map(|(slot, _)| self.encode(&slot.ty, &snake(&slot.name)))
                .collect::<Result<Vec<_>, String>>()?;
            self.output.push_str(&format!(
                "        Self::{variant} {{ {} }} => (program.relation_id({canonical:?}).expect(\"generated relation belongs to this program\"), vec![{}]),\n",
                names.join(", "),
                values.join(", ")
            ));
        }
        self.output.push_str("    } }\n    pub fn from_tuple(tuple: galileo_runtime::Tuple, program: &Program) -> Result<Self, EvalError> {\n        let (relation, values) = tuple;\n        match program.relation(relation).map(|relation| relation.name.as_str()) {\n");
        for (canonical, variant, fields) in &variants {
            self.output.push_str(&format!("        Some({canonical:?}) if values.len() == {} => {{ let mut values = values.iter(); Ok(Self::{variant} {{\n", fields.len()));
            for (slot, ty) in fields {
                self.output.push_str(&format!(
                    "            {}: ({})?,\n",
                    snake(&slot.name),
                    self.decode(
                        &slot.ty,
                        "values.next().ok_or_else(|| invalid(\"tuple\"))?",
                        ty
                    )?
                ));
            }
            self.output.push_str("        }) },\n");
        }
        self.output
            .push_str("        _ => Err(invalid(\"tuple\")),\n    } }\n");
        self.output.push_str("}\n");
        if kind == RelationKind::Event {
            self.output.push_str(&format!("impl galileo_runtime::Event for {name} {{\n    fn tuple(&self, program: &Program) -> galileo_runtime::Tuple {{ self.clone().into_tuple(program) }}\n}}\n"));
        }
        Ok(())
    }

    fn infer(&self, expr: &Expr, variables: &BTreeMap<String, Type>) -> Result<Type, String> {
        Ok(match expr {
            Expr::Value(value) => match value {
                Value::Bool(_) => Type::Bool,
                Value::Int(_) => Type::Int,
                Value::Text(_) => Type::Text,
                Value::Path(_) => Type::Path,
                Value::OptionPath(_) => Type::OptionPath,
                Value::Entity(entity) => Type::entity(&entity.entity_type),
                _ => return Err(format!("unsupported view literal {value:?}")),
            },
            Expr::Variable(variable) => variables
                .get(&variable.0)
                .cloned()
                .ok_or_else(|| format!("untyped variable {}", variable.0))?,
            Expr::Field { value, field } => match self.infer(value, variables)? {
                Type::Entity(name) => self
                    .program
                    .entity_types
                    .iter()
                    .find(|entity| entity.name == name)
                    .and_then(|entity| entity.fields.iter().find(|slot| slot.name == *field))
                    .map(|slot| slot.ty.clone())
                    .ok_or_else(|| format!("unknown field {name}.{field}"))?,
                Type::Record(fields) => fields
                    .get(field)
                    .cloned()
                    .ok_or_else(|| format!("unknown field {field}"))?,
                other => return Err(format!("field of {other:?}")),
            },
            Expr::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|(name, value)| Ok((name.clone(), self.infer(value, variables)?)))
                    .collect::<Result<_, String>>()?,
            ),
            Expr::List(items) => self
                .infer(
                    items.first().ok_or("empty view list has no element type")?,
                    variables,
                )?
                .list(),
            Expr::If { then_value, .. } => self.infer(then_value, variables)?,
            Expr::Unary { value, .. } => self.infer(value, variables)?,
            Expr::Binary { op, left, .. } => match op {
                BinaryOp::Compare(_) | BinaryOp::And | BinaryOp::Or => Type::Bool,
                _ => self.infer(left, variables)?,
            },
            Expr::Call { function, .. } => {
                if let Some(definition) = self
                    .program
                    .definitions
                    .iter()
                    .find(|definition| definition.name == *function)
                {
                    definition.result.clone()
                } else {
                    match function.as_str() {
                        "lowercase" => Type::Text,
                        "contains" => Type::Bool,
                        "length" => Type::Int,
                        _ => return Err(format!("unknown view function {function}")),
                    }
                }
            }
            Expr::Query(query) => {
                let mut scope = variables.clone();
                let mut exact = true;
                for literal in &query.body {
                    match literal {
                        Literal::Positive(atom) => {
                            let relation = self
                                .program
                                .relation(atom.relation)
                                .ok_or("unknown query relation")?;
                            exact &= relation
                                .modes
                                .iter()
                                .any(|mode| matches!(mode, Mode::ExactlyOne { .. }));
                            for (argument, slot) in atom.arguments.iter().zip(&relation.slots) {
                                if let Expr::Variable(variable) = argument {
                                    scope.insert(variable.0.clone(), slot.ty.clone());
                                }
                            }
                        }
                        Literal::Bind { variable, value } => {
                            scope.insert(variable.0.clone(), self.infer(value, &scope)?);
                        }
                        Literal::Member { variable, list } => {
                            if let Type::List(inner) = self.infer(list, &scope)? {
                                scope.insert(variable.0.clone(), *inner);
                            }
                            exact = false;
                        }
                        Literal::Aggregate { variable, .. } => {
                            scope.insert(variable.0.clone(), Type::Int);
                        }
                        _ => {}
                    }
                }
                match &query.kind {
                    QueryKind::The(variable) => {
                        let ty = scope
                            .get(&variable.0)
                            .cloned()
                            .ok_or_else(|| format!("untyped query variable {}", variable.0))?;
                        if exact {
                            ty
                        } else {
                            ty.maybe()
                        }
                    }
                    QueryKind::All(_) if query.head.is_some() => self.infer(query.head.as_ref().unwrap(), &scope)?.list(),
                    QueryKind::All(variables) if variables.len() == 1 => scope
                        .get(&variables[0].0)
                        .cloned()
                        .ok_or("untyped query result")?
                        .list(),
                    QueryKind::All(variables) => Type::Record(
                        variables
                            .iter()
                            .map(|variable| {
                                Ok((
                                    variable.0.clone(),
                                    scope
                                        .get(&variable.0)
                                        .cloned()
                                        .ok_or("untyped query result")?,
                                ))
                            })
                            .collect::<Result<_, String>>()?,
                    )
                    .list(),
                }
            }
            _ => return Err(format!("unsupported view expression {expr:?}")),
        })
    }
}

fn pascal(name: &str) -> String {
    name.rsplit(['/', '.'])
        .next()
        .unwrap_or(name)
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            let first = characters.next().unwrap();
            format!("{}{}", first.to_uppercase(), characters.as_str())
        })
        .collect()
}

fn snake(name: &str) -> String {
    let mut out = String::new();
    for (index, character) in name.chars().enumerate() {
        if !character.is_alphanumeric() {
            if !out.ends_with('_') {
                out.push('_');
            }
        } else {
            if character.is_uppercase() && index > 0 && !out.ends_with('_') {
                out.push('_');
            }
            out.extend(character.to_lowercase());
        }
    }
    if matches!(
        out.as_str(),
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
    ) {
        out.push('_');
    }
    out
}

fn unique(names: impl IntoIterator<Item = String>) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for name in names {
        if name.is_empty()
            || name.starts_with(|character: char| character.is_numeric())
            || !seen.insert(name.clone())
        {
            return Err(format!("invalid or duplicate generated Rust name: {name}"));
        }
    }
    Ok(())
}
