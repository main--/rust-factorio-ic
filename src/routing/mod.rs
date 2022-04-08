use fehler::throws;

use crate::pcb::{Pcb, NeededWires, Entity, Function, Point, Direction, NeededWire, WireKind};

#[cfg(feature = "leemaze_lib")]
mod leemaze_lib;
#[cfg(feature = "leemaze_lib")]
pub use leemaze_lib::lee_pathfinder;

mod mylee;
pub use mylee::{mylee as mylee, Options as MyleeOptions};

use std::convert::TryInto;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use rand::prelude::*;

pub fn route<P: Pcb>(pcb: &mut P, needed_wires: NeededWires, pathfinder_fn: impl Fn(&mut P, &NeededWire) -> Result<(), ()> + Clone + Send + 'static) {
    let desired_results = 10;

    // TODO: dynamic thread count; if 1 then don't spawn anything and just run directly on this thread
    let canceled = Arc::new(AtomicBool::new(false));
    let (tx, rx) = std::sync::mpsc::channel();
    for tid in 0..8 {
        let pcb = pcb.clone();
        let canceled = Arc::clone(&canceled);
        let needed_wires = needed_wires.clone();
        let pathfinder_fn = pathfinder_fn.clone();
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut tid = tid;
            while let Some(result) = route_worker(pcb.clone(), tid, &canceled, needed_wires.clone(), pathfinder_fn.clone()) {
                let _ = tx.send(result);
                tid += 8;
            }
        });
    }

    let mut results_buf = Vec::new();
    while results_buf.len() < desired_results {
        let p = rx.recv().unwrap();
        results_buf.push(p);
        println!("==== SOLUTION#{} ====", results_buf.len());
    }

    canceled.store(true, std::sync::atomic::Ordering::SeqCst);

    for p in &results_buf {
        let mut p2 = p.clone();
        reduce_gratuitous_undergrounds(&mut p2);
        println!("{} / {} entities", p.entities().count(), p2.entities().count());
    }

    *pcb = results_buf.into_iter().min_by_key(|p| p.entities().count()).unwrap();

    reduce_gratuitous_undergrounds(pcb);
}

pub fn route_worker<P: Pcb>(
    pcb: P,
    tid: u64,
    canceled: &AtomicBool,
    mut needed_wires: NeededWires,
    pathfinder_fn: impl Fn(&mut P, &NeededWire) -> Result<(), ()>
) -> Option<P> {
    // simulated annealing-ish to choose wiring order
    let mut panic = 0;
    let mut temperature = 20;

    let mut rng = StdRng::seed_from_u64(tid);
    let mut total_tries = 0;
    let mut total_depth = 0;

    while !canceled.load(std::sync::atomic::Ordering::Relaxed) {
        match try_wiring(pcb.clone(), &needed_wires, &pathfinder_fn) {
            Ok(p) => {
                total_tries += 1;
                total_depth += needed_wires.len();
                println!("[{tid}] total tries: {}", total_tries);
                println!("[{tid}] total depth: {}", total_depth);
                println!("[{tid}] averg depth: {:2}", total_depth as f32 / total_tries as f32);
                return Some(p);
            }
            Err(i) => {
                let ele = needed_wires.remove(i);
                needed_wires.insert(0, ele);

                if panic == temperature {
                    panic = 0;
                    temperature += 1;
                    println!("[{tid}] temp={}", temperature);


                    needed_wires.shuffle(&mut rng);
                }

                total_depth += i + 1;
                total_tries += 1;
                panic += 1;
                //println!("[{tid}] panic={}", panic);
            }
        }
    }

    None
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
    pathfinder_fn: &impl Fn(&mut P, &NeededWire) -> Result<(), ()>,
) -> P {
    for (i, wire) in needed_wires.iter().enumerate() {
        // render_blueprint_ascii(&pcb);
        #[cfg(feature = "render_wiring_steps")]
        println!("{}", render::ascii(&pcb));

        pathfinder_fn(&mut pcb, wire).map_err(|()| i)?;
    }
    pcb
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum LogisticRoute {
    Normal(Direction),
    Underground {
        dir: Direction,
        gap: i32,
    },
}
impl LogisticRoute {
    pub fn direction(&self) -> Direction {
        match *self {
            LogisticRoute::Normal(dir) => dir,
            LogisticRoute::Underground { dir, .. } => dir,
        }
    }
    pub fn position_after(&self, point: Point) -> Point {
        match *self {
            LogisticRoute::Normal(dir) => point + dir.to_vector(),
            LogisticRoute::Underground { dir, gap } => point + (dir.to_vector() * (gap + 2)),
        }
    }
    pub fn underground_belt_end_position(&self, point: Point) -> Option<Point> {
        match *self {
            LogisticRoute::Normal(_) => None,
            LogisticRoute::Underground { dir, gap } => Some(point + (dir.to_vector() * (gap + 1))),
        }
    }
}

fn insert_underground_belts<I: IntoIterator<Item=Direction>>(path: I, gap_limit: usize) -> Vec<LogisticRoute>
    where I::IntoIter: Clone {
    let mut undergrounded_path = Vec::new();
    let mut path = path.into_iter();
    while let Some(current_direction) = path.next() {
        let is_same_direction = match undergrounded_path.last() {
            Some(LogisticRoute::Normal(dir)) => *dir == current_direction,
            Some(LogisticRoute::Underground { dir, .. }) => *dir == current_direction,
            None => false,
        };
        // number of tiles including current going into the same direction
        let tail_length = path.clone().take_while(|&d| d == current_direction).count() + 1;

        if !is_same_direction || tail_length <= 2 {
            undergrounded_path.push(LogisticRoute::Normal(current_direction));
        } else {
            // insert underground belt
            // FIXME: pipes can have distance of 9 instead of 4; also other belt types
            let gap = std::cmp::min(tail_length - 2, gap_limit) as i32;
            undergrounded_path.push(LogisticRoute::Underground { dir: current_direction, gap });
            // skip belts we're replacing
            path.nth(gap.try_into().unwrap()).unwrap();
        }
    }
    undergrounded_path
}


fn apply_lee_path<I: IntoIterator<Item = LogisticRoute>>(pcb: &mut impl Pcb, from: Point, path: I, kind: &WireKind) where I::IntoIter: Clone {
    let mut cursor = from;
    let path = path.into_iter();
    // println!("{}", render::ascii_wire(pcb, from, path.clone(), pcb.entity_rect().pad(1)));
    for (i, belt) in path.enumerate() {
        let mut add_beginning = |x| if i == 0 { pcb.replace(x) } else { pcb.add(x) };

        match belt {
            LogisticRoute::Normal(dir) => {
                let function = match kind {
                    WireKind::Belt => Function::Belt(dir),
                    WireKind::Pipe(ref x) => Function::Pipe(x.clone()),
                };
                add_beginning(Entity { location: cursor, function });
            },
            LogisticRoute::Underground { dir, .. } => {
                let (f1, f2) = match kind {
                    WireKind::Belt => (Function::UndergroundBelt(dir, true), Function::UndergroundBelt(dir, false)),
                    WireKind::Pipe(_) => (Function::UndergroundPipe(dir.opposite_direction()), Function::UndergroundPipe(dir)),
                };
                add_beginning(Entity {
                    location: cursor,
                    function: f1,
                });
                pcb.add(Entity {
                    location: belt.underground_belt_end_position(cursor).unwrap(),
                    function: f2,
                });
            },
        }
        cursor = belt.position_after(cursor);
    }
}
