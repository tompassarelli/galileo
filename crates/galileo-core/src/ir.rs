use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceSpan {
    pub file: String,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Type {
    Any,
    Bool,
    Int,
    Text,
    Path,
    OptionPath,
    Entity(String),
    TextUnion(Vec<String>),
    Maybe(Box<Type>),
    List(Box<Type>),
    Record(BTreeMap<String, Type>),
    Attrs(Box<Type>),
    Nix(Box<Type>),
    Function {
        parameters: Vec<Type>,
        result: Box<Type>,
    },
}

impl Type {
    pub fn entity(name: impl Into<String>) -> Self {
        Self::Entity(name.into())
    }
    pub fn maybe(self) -> Self {
        Self::Maybe(Box::new(self))
    }
    pub fn list(self) -> Self {
        Self::List(Box::new(self))
    }
    pub fn nix(self) -> Self {
        Self::Nix(Box::new(self))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Entity {
    pub name: String,
    pub creation_order: Option<u64>,
    pub entity_type: String,
}

impl Entity {
    pub fn named(entity_type: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            creation_order: None,
            entity_type: entity_type.into(),
        }
    }
    pub fn created(entity_type: impl Into<String>, name: impl Into<String>, id: u64) -> Self {
        Self {
            name: name.into(),
            creation_order: Some(id),
            entity_type: entity_type.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Text(String),
    Path(String),
    OptionPath(Vec<String>),
    Entity(Entity),
    List(Vec<Value>),
    Record(BTreeMap<String, Value>),
    Attrs(BTreeMap<String, Value>),
    Nix { normalized: String },
}

impl Value {
    pub fn entity(entity_type: impl Into<String>, name: impl Into<String>) -> Self {
        Self::Entity(Entity::named(entity_type, name))
    }
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}
impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}
impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}
impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::text(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Variable(pub String);
impl Variable {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
    pub fn is_anonymous(&self) -> bool {
        self.0.starts_with("_#")
    }
}
impl From<&str> for Variable {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelationId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationKind {
    Static,
    State,
    Derived,
    Event,
    Effect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub name: String,
    pub ty: Type,
}
impl Slot {
    pub fn new(name: impl Into<String>, ty: Type) -> Self {
        Self {
            name: name.into(),
            ty,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    ExactlyOne {
        key: usize,
        values: Vec<usize>,
    },
    AtMostOne {
        keys: Vec<usize>,
        values: Vec<usize>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relation {
    pub name: String,
    pub kind: RelationKind,
    pub slots: Vec<Slot>,
    pub template: String,
    pub modes: Vec<Mode>,
    pub checkpoint: bool,
    pub source: Option<SourceSpan>,
}

impl Relation {
    pub fn new(
        name: impl Into<String>,
        kind: RelationKind,
        slots: impl IntoIterator<Item = Slot>,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            slots: slots.into_iter().collect(),
            template: String::new(),
            modes: Vec::new(),
            checkpoint: false,
            source: None,
        }
    }
    pub fn template(mut self, template: impl Into<String>) -> Self {
        self.template = template.into();
        self
    }
    pub fn mode(mut self, mode: Mode) -> Self {
        self.modes.push(mode);
        self
    }
    pub fn checkpoint(mut self) -> Self {
        self.checkpoint = true;
        self
    }
    pub fn at(mut self, source: SourceSpan) -> Self {
        self.source = Some(source);
        self
    }
    pub fn atom(&self, id: RelationId, arguments: impl IntoIterator<Item = Expr>) -> Atom {
        Atom::new(id, arguments)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectLevel {
    Total,
    Staged,
    ReadsState,
    Transition,
    Emits,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Negate,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompareOp {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Concat,
    Merge,
    And,
    Or,
    Compare(CompareOp),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Value(Value),
    Variable(Variable),
    List(Vec<Expr>),
    Record(BTreeMap<String, Expr>),
    Attrs(BTreeMap<String, Expr>),
    Field {
        value: Box<Expr>,
        field: String,
    },
    Unary {
        op: UnaryOp,
        value: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        function: String,
        arguments: Vec<Expr>,
    },
    If {
        condition: Box<Expr>,
        then_value: Box<Expr>,
        else_value: Box<Expr>,
    },
    Lambda {
        parameters: Vec<Slot>,
        body: Box<Expr>,
    },
    Apply {
        function: Box<Expr>,
        arguments: Vec<Expr>,
    },
    Query(Box<Query>),
}

impl Expr {
    pub fn var(name: impl Into<String>) -> Self {
        Self::Variable(Variable::new(name))
    }
    pub fn value(value: impl Into<Value>) -> Self {
        Self::Value(value.into())
    }
    pub fn call(function: impl Into<String>, arguments: impl IntoIterator<Item = Expr>) -> Self {
        Self::Call {
            function: function.into(),
            arguments: arguments.into_iter().collect(),
        }
    }
    pub fn binary(op: BinaryOp, left: Expr, right: Expr) -> Self {
        Self::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        }
    }
}
impl From<Value> for Expr {
    fn from(value: Value) -> Self {
        Self::Value(value)
    }
}
impl From<Query> for Expr {
    fn from(query: Query) -> Self {
        Self::Query(Box::new(query))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Atom {
    pub relation: RelationId,
    pub arguments: Vec<Expr>,
    pub source: Option<SourceSpan>,
}
impl Atom {
    pub fn new(relation: RelationId, arguments: impl IntoIterator<Item = Expr>) -> Self {
        Self {
            relation,
            arguments: arguments.into_iter().collect(),
            source: None,
        }
    }
    pub fn at(mut self, source: SourceSpan) -> Self {
        self.source = Some(source);
        self
    }
    pub fn positive(self) -> Literal {
        Literal::Positive(self)
    }
    pub fn negative(self) -> Literal {
        Literal::Negative(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FactOrigin {
    Stated(Option<SourceSpan>),
    Foreign {
        source: String,
        revision: String,
        location: Option<SourceSpan>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fact {
    pub relation: RelationId,
    pub values: Vec<Value>,
    pub origin: FactOrigin,
}
impl Fact {
    pub fn new(relation: RelationId, values: impl IntoIterator<Item = Value>) -> Self {
        Self {
            relation,
            values: values.into_iter().collect(),
            origin: FactOrigin::Stated(None),
        }
    }
    pub fn at(mut self, source: SourceSpan) -> Self {
        self.origin = FactOrigin::Stated(Some(source));
        self
    }
    pub fn atom(&self) -> Atom {
        Atom::new(self.relation, self.values.iter().cloned().map(Expr::Value))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AggregateOp {
    Count,
    Sum(Expr),
    Min(Expr),
    Max(Expr),
}
impl AggregateOp {
    pub fn value(&self) -> Option<&Expr> {
        match self {
            Self::Count => None,
            Self::Sum(value) | Self::Min(value) | Self::Max(value) => Some(value),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Literal {
    Positive(Atom),
    Negative(Atom),
    Compare {
        op: CompareOp,
        left: Expr,
        right: Expr,
    },
    Boolean(Expr),
    Bind {
        variable: Variable,
        value: Expr,
    },
    Member {
        variable: Variable,
        list: Expr,
    },
    Aggregate {
        variable: Variable,
        op: AggregateOp,
        body: Vec<Literal>,
    },
    ArgMin {
        variable: Variable,
        candidate: Expr,
        key: Expr,
        body: Vec<Literal>,
    },
}
impl Literal {
    pub fn bind(variable: impl Into<Variable>, value: Expr) -> Self {
        Self::Bind {
            variable: variable.into(),
            value,
        }
    }
    pub fn member(variable: impl Into<Variable>, list: Expr) -> Self {
        Self::Member {
            variable: variable.into(),
            list,
        }
    }
    pub fn compare(op: CompareOp, left: Expr, right: Expr) -> Self {
        Self::Compare { op, left, right }
    }
    pub fn aggregate(
        variable: impl Into<Variable>,
        op: AggregateOp,
        body: impl IntoIterator<Item = Literal>,
    ) -> Self {
        Self::Aggregate {
            variable: variable.into(),
            op,
            body: body.into_iter().collect(),
        }
    }
    pub fn argmin(
        variable: impl Into<Variable>,
        candidate: Expr,
        key: Expr,
        body: impl IntoIterator<Item = Literal>,
    ) -> Self {
        Self::ArgMin {
            variable: variable.into(),
            candidate,
            key,
            body: body.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub name: String,
    pub head: Atom,
    pub body: Vec<Literal>,
    pub body_sources: Vec<SourceSpan>,
    pub source: Option<SourceSpan>,
}
impl Rule {
    pub fn new(name: impl Into<String>, head: Atom) -> Self {
        Self {
            name: name.into(),
            head,
            body: Vec::new(),
            body_sources: Vec::new(),
            source: None,
        }
    }
    pub fn when(mut self, body: impl IntoIterator<Item = Literal>) -> Self {
        self.body = body.into_iter().collect();
        self
    }
    pub fn at(mut self, source: SourceSpan) -> Self {
        self.source = Some(source);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueryKind {
    All(Vec<Variable>),
    The(Variable),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    pub kind: QueryKind,
    pub body: Vec<Literal>,
    pub sorted_by: Option<Box<Expr>>,
}
impl Query {
    pub fn all(
        variables: impl IntoIterator<Item = Variable>,
        body: impl IntoIterator<Item = Literal>,
    ) -> Self {
        Self {
            kind: QueryKind::All(variables.into_iter().collect()),
            body: body.into_iter().collect(),
            sorted_by: None,
        }
    }
    pub fn the(variable: impl Into<Variable>, body: impl IntoIterator<Item = Literal>) -> Self {
        Self {
            kind: QueryKind::The(variable.into()),
            body: body.into_iter().collect(),
            sorted_by: None,
        }
    }
    pub fn sorted_by(mut self, value: Expr) -> Self {
        self.sorted_by = Some(Box::new(value));
        self
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedQuery {
    pub name: String,
    pub query: Query,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definition {
    pub name: String,
    pub parameters: Vec<Slot>,
    pub result: Type,
    pub body: Expr,
    pub effect: EffectLevel,
    pub source: Option<SourceSpan>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityType {
    pub name: String,
    pub state: bool,
    pub fields: Vec<Slot>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgumentDomain {
    Only(Vec<Value>),
    Sampled(Vec<Value>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub relation: RelationId,
    pub domains: BTreeMap<usize, ArgumentDomain>,
}
impl Event {
    pub fn new(relation: RelationId) -> Self {
        Self {
            relation,
            domains: BTreeMap::new(),
        }
    }
    pub fn domain(mut self, slot: usize, domain: ArgumentDomain) -> Self {
        self.domains.insert(slot, domain);
        self
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effect {
    pub relation: RelationId,
    pub at_most_one_per_step: bool,
    pub settled_by: Option<RelationId>,
}
impl Effect {
    pub fn new(relation: RelationId) -> Self {
        Self {
            relation,
            at_most_one_per_step: false,
            settled_by: None,
        }
    }
    pub fn at_most_one_per_step(mut self) -> Self {
        self.at_most_one_per_step = true;
        self
    }
    pub fn settled_by(mut self, event: RelationId) -> Self {
        self.settled_by = Some(event);
        self
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Set(Atom),
    Include(Atom),
    Withdraw(Atom),
    Create {
        variable: Variable,
        entity_type: String,
        fields: BTreeMap<String, Expr>,
    },
    Delete(Expr),
    Emit(Atom),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateClaim {
    Holds(Atom),
    Some {
        variables: Vec<Variable>,
        body: Vec<Literal>,
    },
    No {
        variables: Vec<Variable>,
        body: Vec<Literal>,
    },
    Every {
        variables: Vec<Variable>,
        domain: Vec<Literal>,
        body: Vec<Literal>,
    },
    Before(Box<StateClaim>),
    After(Box<StateClaim>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Claim {
    State(StateClaim),
    Eventually(StateClaim),
    Always(StateClaim),
    Never(StateClaim),
    Requires {
        target: Atom,
        condition: Atom,
    },
    NoSinglePointOfFailure {
        claim: Box<Claim>,
        among: Atom,
    },
    EveryTracePasses {
        variables: Vec<Variable>,
        target: StateClaim,
        checkpoint: StateClaim,
    },
    EveryEventWhen {
        event: Atom,
        condition: StateClaim,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Requirement {
    pub name: String,
    pub claim: Claim,
    pub invariant: bool,
    pub source: Option<SourceSpan>,
}
impl Requirement {
    pub fn new(name: impl Into<String>, claim: Claim) -> Self {
        Self {
            name: name.into(),
            claim,
            invariant: false,
            source: None,
        }
    }
    pub fn invariant(name: impl Into<String>, claim: StateClaim) -> Self {
        Self {
            name: name.into(),
            claim: Claim::State(claim),
            invariant: true,
            source: None,
        }
    }
    pub fn at(mut self, source: SourceSpan) -> Self {
        self.source = Some(source);
        self
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct HandlerClause {
    pub body: Vec<Literal>,
    pub body_sources: Vec<SourceSpan>,
    pub actions: Vec<Action>,
    pub ensures: Vec<StateClaim>,
    pub source: Option<SourceSpan>,
}
impl HandlerClause {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn when(mut self, body: impl IntoIterator<Item = Literal>) -> Self {
        self.body = body.into_iter().collect();
        self
    }
    pub fn action(mut self, action: Action) -> Self {
        self.actions.push(action);
        self
    }
    pub fn ensures(mut self, claim: StateClaim) -> Self {
        self.ensures.push(claim);
        self
    }
    pub fn at(mut self, source: SourceSpan) -> Self {
        self.source = Some(source);
        self
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handler {
    pub event: Atom,
    pub clauses: Vec<HandlerClause>,
    pub may_skip: bool,
}
impl Handler {
    pub fn new(event: Atom) -> Self {
        Self {
            event,
            clauses: Vec::new(),
            may_skip: false,
        }
    }
    pub fn clause(mut self, clause: HandlerClause) -> Self {
        self.clauses.push(clause);
        self
    }
    pub fn may_skip(mut self) -> Self {
        self.may_skip = true;
        self
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub name: String,
    pub parameters: Vec<Slot>,
    pub fields: Vec<(String, Expr)>,
    pub source: Option<SourceSpan>,
}
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Program {
    pub entity_types: Vec<EntityType>,
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
    pub facts: Vec<Fact>,
    pub initially: Vec<Fact>,
    pub rules: Vec<Rule>,
    pub queries: Vec<NamedQuery>,
    pub definitions: Vec<Definition>,
    pub events: Vec<Event>,
    pub handlers: Vec<Handler>,
    pub effects: Vec<Effect>,
    pub requirements: Vec<Requirement>,
    pub views: Vec<View>,
}
impl Program {
    pub fn builder() -> ProgramBuilder {
        ProgramBuilder::default()
    }
    pub fn relation(&self, id: RelationId) -> Option<&Relation> {
        self.relations.get(id.0)
    }
    pub fn relation_id(&self, name: &str) -> Option<RelationId> {
        self.relations
            .iter()
            .position(|r| r.name == name)
            .map(RelationId)
    }
    pub fn field_relation(&self, entity_type: &str, field: &str) -> Option<RelationId> {
        if entity_type.contains('/') {
            return self.relation_id(&format!("{entity_type}.{field}"));
        }
        let name = format!("{entity_type}.{field}");
        self.relations
            .iter()
            .position(|r| r.name.rsplit('/').next() == Some(name.as_str()))
            .map(RelationId)
    }
}
#[derive(Clone, Debug, Default)]
pub struct ProgramBuilder {
    program: Program,
}
impl ProgramBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn entity_type(&mut self, name: impl Into<String>) -> &mut Self {
        self.program.entity_types.push(EntityType {
            name: name.into(),
            state: false,
            fields: Vec::new(),
        });
        self
    }
    pub fn state_entity_type(
        &mut self,
        name: impl Into<String>,
        fields: impl IntoIterator<Item = Slot>,
    ) -> &mut Self {
        self.program.entity_types.push(EntityType {
            name: name.into(),
            state: true,
            fields: fields.into_iter().collect(),
        });
        self
    }
    pub fn entity(&mut self, entity: Entity) -> &mut Self {
        self.program.entities.push(entity);
        self
    }
    pub fn relation(&mut self, relation: Relation) -> RelationId {
        let id = RelationId(self.program.relations.len());
        self.program.relations.push(relation);
        id
    }
    pub fn fact(&mut self, fact: Fact) -> &mut Self {
        self.program.facts.push(fact);
        self
    }
    pub fn initially(&mut self, fact: Fact) -> &mut Self {
        self.program.initially.push(fact);
        self
    }
    pub fn rule(&mut self, rule: Rule) -> &mut Self {
        self.program.rules.push(rule);
        self
    }
    pub fn query(&mut self, name: impl Into<String>, query: Query) -> &mut Self {
        self.program.queries.push(NamedQuery {
            name: name.into(),
            query,
        });
        self
    }
    pub fn definition(&mut self, definition: Definition) -> &mut Self {
        self.program.definitions.push(definition);
        self
    }
    pub fn event(&mut self, event: Event) -> &mut Self {
        self.program.events.push(event);
        self
    }
    pub fn handler(&mut self, handler: Handler) -> &mut Self {
        self.program.handlers.push(handler);
        self
    }
    pub fn effect(&mut self, effect: Effect) -> &mut Self {
        self.program.effects.push(effect);
        self
    }
    pub fn requirement(&mut self, requirement: Requirement) -> &mut Self {
        self.program.requirements.push(requirement);
        self
    }
    pub fn view(&mut self, view: View) -> &mut Self {
        self.program.views.push(view);
        self
    }
    pub fn build(self) -> Program {
        self.program
    }
}
