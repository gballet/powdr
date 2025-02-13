pub mod display;
pub mod json_exporter;
pub mod pil_analyzer;
pub mod util;

use std::collections::HashMap;
use std::path::Path;

use number::{DegreeType, FieldElement};
pub use parser::ast::{BinaryOperator, UnaryOperator};

pub fn analyze(path: &Path) -> Analyzed {
    pil_analyzer::process_pil_file(path)
}

pub fn analyze_string(contents: &str) -> Analyzed {
    pil_analyzer::process_pil_file_contents(contents)
}

pub enum StatementIdentifier {
    Definition(String),
    PublicDeclaration(String),
    Identity(usize),
}

pub struct Analyzed {
    /// Constants are not namespaced!
    pub constants: HashMap<String, FieldElement>,
    pub definitions: HashMap<String, (Polynomial, Option<FunctionValueDefinition>)>,
    pub public_declarations: HashMap<String, PublicDeclaration>,
    pub identities: Vec<Identity>,
    /// The order in which definitions and identities
    /// appear in the source.
    pub source_order: Vec<StatementIdentifier>,
}

impl Analyzed {
    /// @returns the number of committed polynomials (with multiplicities for arrays)
    pub fn commitment_count(&self) -> usize {
        self.declaration_type_count(PolynomialType::Committed)
    }
    /// @returns the number of intermediate polynomials (with multiplicities for arrays)
    pub fn intermediate_count(&self) -> usize {
        self.declaration_type_count(PolynomialType::Intermediate)
    }
    /// @returns the number of constant polynomials (with multiplicities for arrays)
    pub fn constant_count(&self) -> usize {
        self.declaration_type_count(PolynomialType::Constant)
    }

    pub fn constant_polys_in_source_order(
        &self,
    ) -> Vec<&(Polynomial, Option<FunctionValueDefinition>)> {
        self.definitions_in_source_order(PolynomialType::Constant)
    }

    pub fn committed_polys_in_source_order(
        &self,
    ) -> Vec<&(Polynomial, Option<FunctionValueDefinition>)> {
        self.definitions_in_source_order(PolynomialType::Committed)
    }

    pub fn definitions_in_source_order(
        &self,
        poly_type: PolynomialType,
    ) -> Vec<&(Polynomial, Option<FunctionValueDefinition>)> {
        self.source_order
            .iter()
            .filter_map(move |statement| {
                if let StatementIdentifier::Definition(name) = statement {
                    let definition = &self.definitions[name];
                    if definition.0.poly_type == poly_type {
                        return Some(definition);
                    }
                }
                None
            })
            .collect()
    }

    fn declaration_type_count(&self, poly_type: PolynomialType) -> usize {
        self.definitions
            .iter()
            .filter_map(move |(_name, (poly, _))| {
                if poly.poly_type == poly_type {
                    Some(poly.length.unwrap_or(1) as usize)
                } else {
                    None
                }
            })
            .sum()
    }
}

pub struct Polynomial {
    pub id: u64,
    pub source: SourceRef,
    pub absolute_name: String,
    pub poly_type: PolynomialType,
    pub degree: DegreeType,
    pub length: Option<DegreeType>,
}

impl Polynomial {
    pub fn is_array(&self) -> bool {
        self.length.is_some()
    }
}

pub enum FunctionValueDefinition {
    Mapping(Expression),
    Array(Vec<RepeatedArray>),
    Query(Expression),
}

/// An array of elements that might be repeated (the whole list is repeated).
pub struct RepeatedArray {
    pub values: Vec<Expression>,
    pub repetitions: DegreeType,
}

impl RepeatedArray {
    /// Returns the number of elements in this array (including repetitions).
    pub fn size(&self) -> DegreeType {
        if self.repetitions == 0 {
            assert!(self.values.is_empty());
        }
        if self.values.is_empty() {
            assert!(self.repetitions <= 1)
        }
        self.values.len() as DegreeType * self.repetitions
    }
}

pub struct PublicDeclaration {
    pub id: u64,
    pub source: SourceRef,
    pub name: String,
    pub polynomial: PolynomialReference,
    /// The evaluation point of the polynomial, not the array index.
    pub index: DegreeType,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Identity {
    /// The ID is specific to the kind.
    pub id: u64,
    pub kind: IdentityKind,
    pub source: SourceRef,
    /// For a simple polynomial identity, the selector contains
    /// the actual expression.
    pub left: SelectedExpressions,
    pub right: SelectedExpressions,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum IdentityKind {
    Polynomial,
    Plookup,
    Permutation,
    Connect,
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct SelectedExpressions {
    pub selector: Option<Expression>,
    pub expressions: Vec<Expression>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Expression {
    Constant(String),
    PolynomialReference(PolynomialReference),
    LocalVariableReference(u64),
    PublicReference(String),
    Number(FieldElement),
    String(String),
    Tuple(Vec<Expression>),
    BinaryOperation(Box<Expression>, BinaryOperator, Box<Expression>),
    UnaryOperation(UnaryOperator, Box<Expression>),
    /// Call to a non-macro function (like a constant polynomial)
    FunctionCall(String, Vec<Expression>),
    MatchExpression(Box<Expression>, Vec<(Option<FieldElement>, Expression)>),
}

#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub struct PolynomialReference {
    // TODO would be better to use numeric IDs instead of names,
    // but the IDs as they are overlap. Maybe we can change that.
    pub name: String,
    pub index: Option<u64>,
    pub next: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PolynomialType {
    Committed,
    Constant,
    Intermediate,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRef {
    pub file: String, // TODO should maybe be a shared pointer
    pub line: usize,
}
