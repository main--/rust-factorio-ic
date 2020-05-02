use fehler::throws;

use crate::render;
use crate::pcb::{Pcb, NeededWires, Entity, Function, Point, Direction};

mod leemaze_lib;
mod mylee;

pub use leemaze_lib::lee_pathfinder;
pub use mylee::mylee as mylee;
use std::convert::TryInto;

pub fn route(pcb: &mut Pcb, needed_wires: &mut NeededWires, pathfinder_fn: fn(&mut Pcb, (i32, i32), (i32, i32)) -> Result<(), ()>) {
    while let Err(i) = try_wiring(pcb.clone(), &needed_wires, pathfinder_fn) {
        let ele = needed_wires.remove(i);
        needed_wires.insert(0, ele);
    }
}

#[throws(usize)]
fn try_wiring(mut pcb: Pcb, needed_wires: &NeededWires, pathfinder_fn: fn(&mut Pcb, (i32, i32), (i32, i32)) -> Result<(), ()>) {
    for (i, &(from, to)) in needed_wires.iter().enumerate() {
        // render_blueprint_ascii(&pcb);
        pathfinder_fn(&mut pcb, from, to).map_err(|()| i)?;
    }

    println!("{}", render::ascii(&pcb));
    println!("{}", render::blueprint(&pcb));
}

enum Belt {
    Normal(Direction),
    Underground {
        dir: Direction,
        gap: i32,
    },
}

fn apply_lee_path<I: IntoIterator<Item = Direction>>(pcb: &mut Pcb, from: Point, path: I) where I::IntoIter: Clone {
    let mut undergrounded_path = Vec::new();
    let mut path = path.into_iter();
    while let Some(current_direction) = path.next() {
        let is_same_direction = match undergrounded_path.last() {
            Some(Belt::Normal(dir)) => *dir == current_direction,
            Some(Belt::Underground { dir, .. }) => *dir == current_direction,
            None => false,
        };
        // number of tiles including current going into the same direction
        let tail_length = path.clone().take_while(|&d| d == current_direction).count() + 1;

        if !is_same_direction || tail_length <= 2 {
            undergrounded_path.push(Belt::Normal(current_direction));
        } else {
            // insert underground belt
            let gap = std::cmp::min(tail_length - 2, 4) as i32;
            undergrounded_path.push(Belt::Underground { dir: current_direction, gap });
            // skip belts we're replacing
            path.nth(gap.try_into().unwrap()).unwrap();
        }
    }

    let mut cursor = from;
    for belt in undergrounded_path {
        match belt {
            Belt::Normal(dir) => {
                pcb.replace(Entity { location: cursor, function: Function::Belt(dir) });
                cursor += dir.to_vector();
            },
            Belt::Underground { dir, gap } => {
                pcb.replace(Entity {
                    location: cursor,
                    function: Function::UndergroundBelt(dir, true),
                });
                pcb.add(Entity {
                    location: cursor + dir.to_vector() * (gap + 1),
                    function: Function::UndergroundBelt(dir, false),
                });

                cursor += dir.to_vector() * (gap + 2);
            },
        }
    }
}