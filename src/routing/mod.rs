use fehler::throws;

use crate::render;
use crate::pcb::{Pcb, NeededWires, Entity, Function, Point, Direction};

mod leemaze_lib;
mod mylee;

pub use leemaze_lib::lee_pathfinder;
pub use mylee::mylee as mylee;
use std::convert::TryInto;
use rand::prelude::*;

bitflags::bitflags! {
    pub struct RoutingOptimizations: u64 {
        const MYLEE_PREFER_SAME_DIRECTION = 0b00000001;
        const MYLEE_USE_UNDERGROUND_BELTS = 0b00000010;
    }
}

pub fn route(pcb: &mut Pcb, needed_wires: &mut NeededWires, pathfinder_fn: fn(&mut Pcb, (i32, i32), (i32, i32), RoutingOptimizations) -> Result<(), ()>, optimizations: RoutingOptimizations) {
    // simulated annealing-ish to choose wiring order
    let mut panic = 0;
    let mut temperature = 20;
    let mut rng = StdRng::from_seed([0; 32]);
    let mut total_tries = 0;
    let mut total_depth = 0;

    loop {
        match try_wiring(pcb.clone(), &needed_wires, pathfinder_fn, optimizations) {
            Ok(p) => {
                *pcb = p;
                total_tries += 1;
                total_depth += needed_wires.len();
                println!("total tries: {}", total_tries);
                println!("total depth: {}", total_depth);
                println!("averg depth: {:2}", total_depth as f32 / total_tries as f32);
                return;
            }
            Err(i) => {
                let ele = needed_wires.remove(i);
                needed_wires.insert(0, ele);

                if panic == temperature {
                    panic = 0;
                    temperature += 1;

                    needed_wires.shuffle(&mut rng);
                }

                total_depth += i + 1;
                total_tries += 1;
                panic += 1;
                println!("panic={}", panic);
            }
        }
    }
}

#[throws(usize)]
fn try_wiring(mut pcb: Pcb,
    needed_wires: &NeededWires,
    pathfinder_fn: fn(&mut Pcb, (i32, i32), (i32, i32), RoutingOptimizations) -> Result<(), ()>,
    opts: RoutingOptimizations,
) -> Pcb {
    for (i, &(from, to)) in needed_wires.iter().enumerate() {
        // render_blueprint_ascii(&pcb);
        #[cfg(feature = "render_wiring_steps")]
        println!("{}", render::ascii(&pcb));

        pathfinder_fn(&mut pcb, from, to, opts).map_err(|()| i)?;
    }
    pcb
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
enum Belt {
    Normal(Direction),
    Underground {
        dir: Direction,
        gap: i32,
    },
}
impl Belt {
    fn direction(&self) -> Direction {
        match *self {
            Belt::Normal(dir) => dir,
            Belt::Underground { dir, .. } => dir,
        }
    }
}

fn insert_underground_belts<I: IntoIterator<Item = Belt>>(path: I) -> Vec<Belt> where I::IntoIter: Clone {
    let mut undergrounded_path = Vec::new();
    let mut path = path.into_iter();
    while let Some(belt) = path.next() {
        match belt {
            Belt::Underground { .. } => undergrounded_path.push(belt),
            Belt::Normal(_) => {
                // check if we could underground it
                let is_same_direction = match undergrounded_path.last() {
                    Some(Belt::Normal(dir)) => *dir == belt.direction(),
                    Some(Belt::Underground { dir, .. }) => *dir == belt.direction(),
                    None => false,
                };
                // number of tiles including current going into the same direction
                let tail_length = path.clone()
                    .take_while(|&belt| match belt {
                        Belt::Normal(dir) if dir == belt.direction() => true,
                        _ => false,
                    }).count() + 1;

                if !is_same_direction || tail_length <= 2 {
                    undergrounded_path.push(Belt::Normal(belt.direction()));
                } else {
                    // insert underground belt
                    let gap = std::cmp::min(tail_length - 2, 4) as i32;
                    undergrounded_path.push(Belt::Underground { dir: belt.direction(), gap });
                    // skip belts we're replacing
                    path.nth(gap.try_into().unwrap()).unwrap();
                }
            }
        }
    }
    undergrounded_path
}

fn apply_lee_path<I: IntoIterator<Item = Belt>>(pcb: &mut Pcb, from: Point, path: I) where I::IntoIter: Clone {
    let mut cursor = from;
    for belt in path {
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
