//! Which agent is being interviewed.
//!
//! Mirrors the useful half of [`telemetry::Subject`] without depending on it —
//! this crate is a leaf, and the three kinds you can interview are not the five
//! kinds the log records: a traveller is a vehicle, and the incident-wide
//! `command` row is not anybody.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubjectKind {
    Household,
    Person,
    Unit,
}

impl SubjectKind {
    /// The tag `telemetry` stores, so a caller can hand a subject to both
    /// without a second mapping.
    pub fn wire(self) -> &'static str {
        match self {
            SubjectKind::Household => "household",
            SubjectKind::Person => "person",
            SubjectKind::Unit => "unit",
        }
    }

    pub fn from_wire(s: &str) -> Option<SubjectKind> {
        match s {
            "household" => Some(SubjectKind::Household),
            "person" => Some(SubjectKind::Person),
            "unit" => Some(SubjectKind::Unit),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SubjectKind::Household => "Household",
            SubjectKind::Person => "Person",
            SubjectKind::Unit => "Unit",
        }
    }
}

/// One interviewable agent: a kind and its index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubjectRef {
    pub kind: SubjectKind,
    pub id: i64,
}

impl SubjectRef {
    pub fn new(kind: SubjectKind, id: i64) -> SubjectRef {
        SubjectRef { kind, id }
    }
}
