use fehler::throws;

use crate::render;
use crate::pcb::{Pcb, NeededWires};

mod leemaze_lib;
mod mylee;

pub use leemaze_lib::lee_pathfinder;

pub fn route(pcb: &mut Pcb, needed_wires: &mut NeededWires, pathfinder_fn: fn(&mut Pcb, (i32, i32), (i32, i32)) -> Result<(), ()>) {
    while let Err(i) = try_wiring(pcb.clone(), needed_wires.clone(), pathfinder_fn) {
        let ele = needed_wires.remove(i);
        needed_wires.insert(0, ele);
    }
}

#[throws(usize)]
fn try_wiring(mut pcb: Pcb, needed_wires: NeededWires, pathfinder_fn: fn(&mut Pcb, (i32, i32), (i32, i32)) -> Result<(), ()>) {
    for (i, (from, to)) in needed_wires.into_iter().enumerate() {
        // render_blueprint_ascii(&pcb);
        pathfinder_fn(&mut pcb, from, to).map_err(|()| i)?;
    }

    println!("{}", render::ascii(&pcb));
    println!("{}", render::blueprint(&pcb));
}
