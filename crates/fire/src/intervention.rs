//! What crews and engines actually do to the fire.
//!
//! Both routes into the core already exist as boundary-condition fields, so
//! suppression is expressed in the model's own terms rather than bolted on:
//! `vegetation_changes` removes fuel, `additional_moisture` wets it.

use scenario::Cell;

#[derive(Debug, Clone, Copy)]
pub enum InterventionKind {
    /// Fuel cleared to bare ground -- hand line, dozer line, or an existing
    /// break being widened. Permanent for the rest of the run.
    Fireline,
    /// Water or retardant applied to the cell. The core decays added moisture
    /// over time on its own, so this is a temporary effect.
    Water { litres_per_m2: f64 },
}

#[derive(Debug, Clone)]
pub struct Intervention {
    pub kind: InterventionKind,
    pub cells: Vec<Cell>,
}

impl Intervention {
    pub fn fireline(cells: Vec<Cell>) -> Intervention {
        Intervention { kind: InterventionKind::Fireline, cells }
    }

    /// A typical engine lays down on the order of 5 mm over the cells it
    /// works, which is a meaningful but not decisive moisture bump.
    pub fn water(cells: Vec<Cell>, litres_per_m2: f64) -> Intervention {
        Intervention { kind: InterventionKind::Water { litres_per_m2 }, cells }
    }
}
