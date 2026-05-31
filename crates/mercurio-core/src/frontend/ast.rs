use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedName {
    pub segments: Vec<String>,
    pub span: SourceSpan,
}

impl QualifiedName {
    pub fn as_colon_string(&self) -> String {
        self.segments.join("::")
    }

    pub fn as_dot_string(&self) -> String {
        self.segments.join(".")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Expr {
    Literal(LiteralExpr),
    Name(QualifiedName),
    SelfRef(SourceSpan),
    Tuple {
        items: Vec<Expr>,
        span: SourceSpan,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: SourceSpan,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        span: SourceSpan,
    },
    Path {
        root: Box<Expr>,
        segment: String,
        span: SourceSpan,
    },
    Call {
        function: String,
        args: Vec<Expr>,
        span: SourceSpan,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiteralExpr {
    Integer(i64),
    Real(String),
    Boolean(bool),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Negate,
    Not,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportDecl {
    pub path: QualifiedName,
    pub docs: Vec<String>,
    pub modifiers: Vec<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiplicityRange {
    pub lower: String,
    pub upper: String,
    pub raw: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartUsageDecl {
    pub name: String,
    pub is_implicit_name: bool,
    pub ty: Option<QualifiedName>,
    pub multiplicity: Option<MultiplicityRange>,
    pub expression: Option<Expr>,
    pub additional_types: Vec<QualifiedName>,
    pub specializes: Vec<QualifiedName>,
    pub subsets: Vec<QualifiedName>,
    pub redefines: Vec<QualifiedName>,
    pub body_members: Vec<Declaration>,
    pub docs: Vec<String>,
    pub modifiers: Vec<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartDefinitionDecl {
    pub name: String,
    pub specializes: Vec<QualifiedName>,
    pub members: Vec<Declaration>,
    pub part_members: Vec<PartUsageDecl>,
    pub docs: Vec<String>,
    pub modifiers: Vec<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenericDefinitionDecl {
    pub keyword: String,
    pub name: String,
    pub specializes: Vec<QualifiedName>,
    pub members: Vec<Declaration>,
    pub docs: Vec<String>,
    pub modifiers: Vec<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenericUsageDecl {
    pub keyword: String,
    pub name: String,
    pub is_implicit_name: bool,
    pub ty: Option<QualifiedName>,
    pub reference_target: Option<QualifiedName>,
    pub allocation_source: Option<QualifiedName>,
    pub allocation_target: Option<QualifiedName>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata_properties: BTreeMap<String, String>,
    pub multiplicity: Option<MultiplicityRange>,
    pub expression: Option<Expr>,
    pub additional_types: Vec<QualifiedName>,
    pub specializes: Vec<QualifiedName>,
    pub subsets: Vec<QualifiedName>,
    pub redefines: Vec<QualifiedName>,
    pub body_members: Vec<Declaration>,
    pub docs: Vec<String>,
    pub modifiers: Vec<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AliasDecl {
    pub name: String,
    pub target: QualifiedName,
    pub docs: Vec<String>,
    pub modifiers: Vec<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Declaration {
    Package(PackageDecl),
    Import(ImportDecl),
    PartDefinition(PartDefinitionDecl),
    PartUsage(PartUsageDecl),
    GenericDefinition(GenericDefinitionDecl),
    GenericUsage(GenericUsageDecl),
    Alias(AliasDecl),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageDecl {
    pub name: QualifiedName,
    pub members: Vec<Declaration>,
    pub imports: Vec<ImportDecl>,
    pub definitions: Vec<PartDefinitionDecl>,
    pub docs: Vec<String>,
    pub modifiers: Vec<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ParsedModule {
    pub package: Option<PackageDecl>,
    pub members: Vec<Declaration>,
    pub imports: Vec<ImportDecl>,
    pub definitions: Vec<PartDefinitionDecl>,
}

pub type SysmlModule = ParsedModule;
