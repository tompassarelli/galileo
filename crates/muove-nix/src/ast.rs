#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Uri(String),
    Ident(String),
    Str(Vec<StrPart>),
    IndentedStr(Vec<StrPart>),
    Path(String),
    List(Vec<Expr>),
    AttrSet { rec: bool, bindings: Vec<Binding> },
    Select { expr: Box<Expr>, path: AttrPath, default: Option<Box<Expr>> },
    HasAttr { expr: Box<Expr>, path: AttrPath },
    Apply { func: Box<Expr>, arg: Box<Expr> },
    Lambda { param: Param, body: Box<Expr> },
    Let { bindings: Vec<Binding>, body: Box<Expr> },
    If { cond: Box<Expr>, then: Box<Expr>, else_: Box<Expr> },
    With { scope: Box<Expr>, body: Box<Expr> },
    Assert { cond: Box<Expr>, body: Box<Expr> },
    BinOp { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> },
    Not(Box<Expr>),
    Neg(Box<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum StrPart {
    Text(String),
    Interp(Expr),
}

pub type AttrPath = Vec<AttrKey>;

#[derive(Clone, Debug, PartialEq)]
pub enum AttrKey {
    Ident(String),
    Str(Vec<StrPart>),
    Dynamic(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Binding {
    Attr { path: AttrPath, value: Expr },
    Inherit { from: Option<Expr>, names: Vec<String> },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Param {
    Ident(String),
    Formals { formals: Vec<Formal>, ellipsis: bool, at: Option<String> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Formal {
    pub name: String,
    pub default: Option<Expr>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Concat,
    Mul,
    Div,
    Add,
    Sub,
    Update,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    And,
    Or,
    Impl,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Concat => "++",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Update => "//",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::And => "&&",
            BinOp::Or => "||",
            BinOp::Impl => "->",
        }
    }

    pub fn prec(self) -> u8 {
        match self {
            BinOp::Concat => 5,
            BinOp::Mul | BinOp::Div => 6,
            BinOp::Add | BinOp::Sub => 7,
            BinOp::Update => 9,
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 10,
            BinOp::Eq | BinOp::Ne => 11,
            BinOp::And => 12,
            BinOp::Or => 13,
            BinOp::Impl => 14,
        }
    }

    pub fn assoc(self) -> Assoc {
        match self {
            BinOp::Concat | BinOp::Update | BinOp::Impl => Assoc::Right,
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge | BinOp::Eq | BinOp::Ne => Assoc::None,
            _ => Assoc::Left,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Assoc {
    Left,
    Right,
    None,
}

pub fn ident(name: &str) -> Expr {
    Expr::Ident(name.to_string())
}

pub fn text(s: &str) -> Expr {
    Expr::Str(vec![StrPart::Text(s.to_string())])
}

pub fn path(p: &str) -> Expr {
    Expr::Path(p.to_string())
}

pub fn attrpath(dotted: &str) -> AttrPath {
    dotted.split('.').map(|k| AttrKey::Ident(k.to_string())).collect()
}

pub fn select(expr: Expr, dotted: &str) -> Expr {
    Expr::Select { expr: Box::new(expr), path: attrpath(dotted), default: None }
}

pub fn apply(func: Expr, arg: Expr) -> Expr {
    Expr::Apply { func: Box::new(func), arg: Box::new(arg) }
}

pub fn attr(dotted: &str, value: Expr) -> Binding {
    Binding::Attr { path: attrpath(dotted), value }
}

pub fn attrs(bindings: Vec<Binding>) -> Expr {
    Expr::AttrSet { rec: false, bindings }
}
