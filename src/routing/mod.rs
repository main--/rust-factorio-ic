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
        const MYLEE_VISITED_WITH_DIRECTIONS = 0b00000100;
    }
}

pub fn route<P: Pcb>(pcb: &mut P, mut needed_wires: NeededWires, pathfinder_fn: impl Fn(&mut P, Point, Point) -> Result<(), ()>) {
    // simulated annealing-ish to choose wiring order
    let mut panic = 0;
    let mut temperature = 20;
    let mut rng = StdRng::from_seed([0; 32]);
    let mut total_tries = 0;
    let mut total_depth = 0;

    loop {
        match try_wiring(pcb.clone(), &needed_wires, &pathfinder_fn) {
            Ok(p) => {
                *pcb = p;
                total_tries += 1;
                total_depth += needed_wires.len();
                println!("total tries: {}", total_tries);
                println!("total depth: {}", total_depth);
                println!("averg depth: {:2}", total_depth as f32 / total_tries as f32);
                reduce_gratuitous_undergrounds(pcb);
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

fn reduce_gratuitous_undergrounds(pcb: &mut impl Pcb) {
    collapse_underground_oneway(pcb, true);
    collapse_underground_oneway(pcb, false);
}
fn collapse_underground_oneway(pcb: &mut impl Pcb, down: bool) {
    let candidates: Vec<_> = pcb.entities().filter_map(|e| match e.function {
        Function::UndergroundBelt(d, mode) if mode == down => Some((e.location, d)),
        _ => None,
    }).collect();

    for (mut pos, dir) in candidates {
        let v = dir.to_vector() * if down { 1 } else { -1 };
        loop {
            let collapse_fully = match pcb.entity_at(pos + v) {
                None => false,
                Some(Entity { function: Function::UndergroundBelt(od, mode), .. }) if *od == dir && *mode != down => true,

                _ => break,
            };

            // collapse the entry by one tile and loop
            pcb.replace(Entity { location: pos, function: Function::Belt(dir) });
            pos += v;
            if collapse_fully {
                pcb.replace(Entity { location: pos, function: Function::Belt(dir) });
                break;
            } else {
                pcb.replace(Entity { location: pos, function: Function::UndergroundBelt(dir, down) });
            }
        }
    }
}


#[throws(usize)]
fn try_wiring<P: Pcb>(mut pcb: P,
    needed_wires: &NeededWires,
    pathfinder_fn: &impl Fn(&mut P, Point, Point) -> Result<(), ()>,
) -> P {
    for (i, &(from, to)) in needed_wires.iter().enumerate() {
        // render_blueprint_ascii(&pcb);
        #[cfg(feature = "render_wiring_steps")]
        println!("{}", render::ascii(&pcb));

        pathfinder_fn(&mut pcb, from, to).map_err(|()| i)?;
    }
    pcb
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Belt {
    Normal(Direction),
    Underground {
        dir: Direction,
        gap: i32,
    },
}
impl Belt {
    pub fn direction(&self) -> Direction {
        match *self {
            Belt::Normal(dir) => dir,
            Belt::Underground { dir, .. } => dir,
        }
    }
    pub fn position_after(&self, point: Point) -> Point {
        match *self {
            Belt::Normal(dir) => point + dir.to_vector(),
            Belt::Underground { dir, gap } => point + (dir.to_vector() * (gap + 2)),
        }
    }
    pub fn underground_belt_end_position(&self, point: Point) -> Option<Point> {
        match *self {
            Belt::Normal(_) => None,
            Belt::Underground { dir, gap } => Some(point + (dir.to_vector() * (gap + 1))),
        }
    }
}

fn insert_underground_belts<I: IntoIterator<Item=Direction>>(path: I) -> Vec<Belt>
    where I::IntoIter: Clone {
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
    undergrounded_path
}


fn apply_lee_path<I: IntoIterator<Item = Belt>>(pcb: &mut impl Pcb, from: Point, path: I) where I::IntoIter: Clone {
    let mut cursor = from;
    let path = path.into_iter();
    // println!("{}", render::ascii_wire(pcb, from, path.clone(), pcb.entity_rect().pad(1)));
    for (i, belt) in path.enumerate() {
        let mut add_beginning = |x| if i == 0 { pcb.replace(x) } else { pcb.add(x) };

        match belt {
            Belt::Normal(dir) => {
                add_beginning(Entity { location: cursor, function: Function::Belt(dir) });
            },
            Belt::Underground { dir, .. } => {
                add_beginning(Entity {
                    location: cursor,
                    function: Function::UndergroundBelt(dir, true),
                });
                pcb.add(Entity {
                    location: belt.underground_belt_end_position(cursor).unwrap(),
                    function: Function::UndergroundBelt(dir, false),
                });
            },
        }
        cursor = belt.position_after(cursor);
    }
}
