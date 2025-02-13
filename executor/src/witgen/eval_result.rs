use std::fmt;

use number::FieldElement;

use super::bit_constraints::BitConstraint;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IncompleteCause {
    /// Previous value of witness column not known when trying to derive a value in the next row. Example: `x' = x` where `x` is unknown
    PreviousValueUnknown(String),
    /// Some parts of an expression are not bit constrained. Example: `x + y == 0x3` with `x | 0x1`. Arguments: the indices of the unconstrained variables.
    BitUnconstrained(Vec<usize>),
    /// Some bit constraints are overlapping. Example: `x + y == 0x3` with `x | 0x3` and `y | 0x3`
    OverlappingBitConstraints,
    /// Multiple rows match a lookup query. Example: `{x, 1} in [{1, 1}, {2, 1}]`
    MultipleLookupMatches,
    /// A linear constraint does not have a unique solution. Example: `x + y == 0`
    MultipleLinearSolutions,
    /// No progress transferring. TODO: not sure this could not be coverred by other cases
    NoProgressTransferring,
    /// Quadratic term found trying to detect an affine expression. Example: `a*b + 2c + d`
    QuadraticTerm,
    /// Division term found trying to detect an affine expression. Example: `a/b + 2c + d`
    DivisionTerm,
    /// Exponentiation term found trying to detect an affine expression. Example: `a**b + 2c + d`
    ExponentiationTerm,
    /// No query answer. Example: we ask the prover for `a` and receive no value. Arguments: the query and the column name
    NoQueryAnswer(String, String),
    /// Query match scrutinee is not constant, so the query fails. Example: evaluate `match x { 1 => 1, _ => 0}` but `x` is not constant.
    NonConstantQueryMatchScrutinee,
    /// The left selector in a lookup is not constant. Example: `x * {1} in [{1}]` where `x` is not constant.
    NonConstantLeftSelector,
    /// A value to be written is not constant. TODO: should this be covered by another case? it's used for memory
    NonConstantWriteValue,
    /// An expression cannot be evaluated.
    ExpressionEvaluationUnimplemented(String),
    /// A value is not found on the left side of a match. Example: `match x {1 => 2, 3 => 4}` where `x == 0`
    NoMatchArmFound,
    /// Last resort error when all possible solving approaches have failed. TODO: make this more precise or use another variant
    SolvingFailed,
    /// Some knowledge was learnt, but not a concrete value. Example: `Y = X` if we know that `Y` is boolean. We learn that `X` is boolean, but not its exact value.
    NotConcrete,
    Multiple(Vec<IncompleteCause>),
}

impl IncompleteCause {
    pub fn combine(self, right: IncompleteCause) -> IncompleteCause {
        match (self, right) {
            (IncompleteCause::Multiple(l), IncompleteCause::Multiple(r)) => {
                IncompleteCause::Multiple(l.into_iter().chain(r).collect())
            }
            (m @ IncompleteCause::Multiple(_), other)
            | (other, m @ IncompleteCause::Multiple(_)) => {
                m.combine(IncompleteCause::Multiple(vec![other]))
            }
            (l, r) => IncompleteCause::Multiple(vec![l, r]),
        }
    }
}

pub type Constraints = Vec<(usize, Constraint)>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvalStatus {
    Complete,
    Incomplete(IncompleteCause),
}

impl From<IncompleteCause> for EvalStatus {
    fn from(value: IncompleteCause) -> Self {
        Self::Incomplete(value)
    }
}

impl EvalStatus {
    pub fn combine<C: Into<EvalStatus>>(self, other: C) -> Self {
        use self::EvalStatus::*;
        let other = other.into();
        match (self, other) {
            (Complete, Complete) => Complete,
            (Incomplete(left), Incomplete(right)) => Incomplete(left.combine(right)),
            (Complete, Incomplete(i)) | (Incomplete(i), Complete) => Incomplete(i),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalValue {
    pub constraints: Constraints,
    pub status: EvalStatus,
}

impl EvalValue {
    pub fn is_complete(&self) -> bool {
        self.status == EvalStatus::Complete
    }

    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    pub fn incomplete(cause: IncompleteCause) -> Self {
        Self::new(vec![], EvalStatus::Incomplete(cause))
    }

    pub fn incomplete_with_constraints(
        constraints: impl IntoIterator<Item = (usize, Constraint)>,
        cause: IncompleteCause,
    ) -> Self {
        Self::new(constraints, EvalStatus::Incomplete(cause))
    }

    pub fn complete(constraints: impl IntoIterator<Item = (usize, Constraint)>) -> Self {
        Self::new(constraints, EvalStatus::Complete)
    }

    fn new(
        constraints: impl IntoIterator<Item = (usize, Constraint)>,
        complete: EvalStatus,
    ) -> Self {
        Self {
            constraints: constraints.into_iter().collect(),
            status: complete,
        }
    }

    pub fn combine(&mut self, other: Self) {
        self.constraints.extend(other.constraints);
        self.status = self.status.clone().combine(other.status);
    }
}

/// Result of evaluating an expression / lookup.
/// New assignments or constraints for witness columns identified by an ID.
pub type EvalResult = Result<EvalValue, EvalError>;

#[derive(Clone, Debug, PartialEq)]
pub enum EvalError {
    /// We ran out of rows
    RowsExhausted,
    /// A constraint that cannot be satisfied (i.e. 2 = 1).
    ConstraintUnsatisfiable(String),
    /// Conflicting bit-constraints in an equation, i.e. for X = 0x100, where X is known to be at most 0xff.
    ConflictingBitConstraints,
    // Fixed lookup failed
    FixedLookupFailed,
    Generic(String),
    Multiple(Vec<EvalError>),
}

impl From<String> for EvalError {
    fn from(value: String) -> Self {
        Self::Generic(value)
    }
}

impl EvalError {
    pub fn combine(self, other: EvalError) -> EvalError {
        match (self, other) {
            (EvalError::Multiple(l), EvalError::Multiple(r)) => {
                EvalError::Multiple(l.into_iter().chain(r).collect())
            }
            (m @ EvalError::Multiple(_), other) | (other, m @ EvalError::Multiple(_)) => {
                m.combine(EvalError::Multiple(vec![other]))
            }
            (l, r) => EvalError::Multiple(vec![l, r]),
        }
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::ConstraintUnsatisfiable(e) => {
                write!(f, "Linear constraint is not satisfiable: {e}",)
            }
            EvalError::Multiple(errors) => {
                for e in errors {
                    write!(f, "{e}")?;
                }
                write!(f, "")
            }
            EvalError::ConflictingBitConstraints => {
                write!(f, "Bit constraints in the expression are conflicting or do not match the constant / offset.",)
            }
            EvalError::RowsExhausted => write!(f, "Table rows exhausted"),
            EvalError::FixedLookupFailed => write!(f, "Lookup into fixed columns failed: no match"),
            EvalError::Generic(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constraint {
    Assignment(FieldElement),
    BitConstraint(BitConstraint),
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Constraint::Assignment(a) => write!(f, " = {a}"),
            Constraint::BitConstraint(bc) => write!(f, ":& {bc}"),
        }
    }
}
