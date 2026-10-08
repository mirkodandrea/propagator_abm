//! What the player decides (01-SPEC-GIOCO §1): an ordered list of places to
//! defend, and per district an explicit civil order. Nothing about units.

use anyhow::{ensure, Result};

/// The civil order for one district. Escalates only: an evacuation is not
/// taken back, and a pre-alert is not withdrawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Civil {
    #[default]
    Nessuno,
    /// Inform and get ready; nobody is told to leave.
    Preallerta,
    /// Leave now.
    Evacua,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    /// District indices, most important first. Districts not listed are not
    /// defended.
    pub priorities: Vec<usize>,
    /// One per district.
    pub civil: Vec<Civil>,
}

impl Plan {
    pub fn new(districts: usize) -> Plan {
        Plan { priorities: vec![], civil: vec![Civil::Nessuno; districts] }
    }

    pub fn with_priorities(mut self, p: &[usize]) -> Plan {
        self.priorities = p.to_vec();
        self
    }

    pub fn with_civil(mut self, d: usize, c: Civil) -> Plan {
        self.civil[d] = c;
        self
    }

    pub fn validate(&self, districts: usize) -> Result<()> {
        ensure!(self.civil.len() == districts, "piano: {} ordini civili per {} quartieri", self.civil.len(), districts);
        for (k, &d) in self.priorities.iter().enumerate() {
            ensure!(d < districts, "piano: quartiere {d} inesistente");
            ensure!(!self.priorities[..k].contains(&d), "piano: quartiere {d} ripetuto nelle priorità");
        }
        Ok(())
    }

    /// The plan as it can actually stand after `active`: civil orders never
    /// step down.
    pub fn escalated_from(&self, active: &Plan) -> Plan {
        let civil = self.civil.iter().zip(&active.civil).map(|(a, b)| (*a).max(*b)).collect();
        Plan { priorities: self.priorities.clone(), civil }
    }

    /// The district's position in the ranking, 0 = first.
    pub fn rank(&self, d: usize) -> Option<usize> {
        self.priorities.iter().position(|&x| x == d)
    }
}
