mod simple_grid;
mod bus;

use crate::kirkmcdonald::ProductionGraph;
use crate::pcb::{Pcb, NeededWires};


pub trait Placer {
    fn place(pcb: &mut impl Pcb, tree: &ProductionGraph) -> NeededWires;
}

pub use simple_grid::SimpleGridPlacer;
pub use bus::BusPlacer;

