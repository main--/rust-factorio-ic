use std::collections::HashSet;
use std::iter;

use fehler::throws;
use either::Either;

use crate::pcb::{Direction, Pcb, Point, Vector, ALL_DIRECTIONS, Entity, Function};
use crate::render;
use crate::routing::{apply_lee_path, RoutingOptimizations, Belt, insert_underground_belts};

// # Types of wires:
//
// One-to-many: One gears assembler feeds many automation science pack assemblers
// Trivial implementation: Belt connection
// Many-to-one
// Trivial implementation: Belt connection
// Lane merge
// Trivial implementation: L+R construction

#[throws(())]
pub fn mylee(pcb: &mut Pcb, from: (i32, i32), to: (i32, i32), opts: RoutingOptimizations) {
    let from = Point::new(from.0, from.1);
    let to = Point::new(to.0, to.1);

    let path = mylee_internal(pcb, &ALL_DIRECTIONS, from, to, opts).ok_or(())?;

    apply_lee_path(pcb, from, path);
}

struct Mazewalker {
    pos: Point,
    history: Vec<Belt>,
}

fn mylee_internal(
    pcb: &Pcb, moveset: &[Direction], from: Point, to: Point, opts: RoutingOptimizations
) -> Option<Vec<Belt>> {
    // ensure enough space around possible entities to possibly lay a belt around everything,
    // including a possible underground belt out, followed by an underground belt back in
    // and the connection loop
    let mut bounds = pcb.entity_rect();
    bounds.a += Vector::new(-2, -2);
    bounds.b += Vector::new(2, 2);

    let mut visited_fields = HashSet::new();
    let mut visited_field_directions = HashSet::new();

    // TODO: there's probably a much better algorithm based around some kind of cost heuristic
    let mut walkers = vec![Mazewalker { pos: from, history: Vec::new() }];
    while !walkers.is_empty() {
       // println!("{} walkers {} visited", walkers.len(), visited_fields.len());

        for walker in std::mem::replace(&mut walkers, Vec::new()) {
           // println!("{} vs {}", walker.pos, to);

            let base_moveset = match walker.history.last() {
                Some(Belt::Underground { dir, .. }) => Either::Left(ALL_DIRECTIONS.iter().filter(move |d| **d != dir.opposite_direction())),
                Some(Belt::Normal(_)) | None => Either::Right(moveset.iter()),
            };

            let prefer_direction =  if opts.contains(RoutingOptimizations::MYLEE_PREFER_SAME_DIRECTION) {
                walker.history.last().map(Belt::direction)
            } else {
                None
            };

            for dir in prefer_direction.into_iter().chain(base_moveset.copied()) {
                let goto = walker.pos + dir.to_vector();
                if goto == to {
                    let mut walker = walker;
                    walker.history.push(Belt::Normal(dir));
                    let mut path = walker.history;
                    if !opts.contains(RoutingOptimizations::MYLEE_USE_UNDERGROUND_BELTS) {
                        path = insert_underground_belts(path);
                    }
                    return Some(path);
                }
                if pcb.is_blocked(goto) {
                    // blocked with existing entity
                    continue;
                }
                if opts.contains(RoutingOptimizations::MYLEE_USE_UNDERGROUND_BELTS) {
                    if visited_field_directions.contains(&(goto, dir)) {
                        continue;
                    }
                } else {
                    if visited_fields.contains(&goto) {
                        continue;
                    }
                }
                if !bounds.contains(goto) {
                    continue;
                }

                visited_fields.insert(goto.clone());
                visited_field_directions.insert((goto, dir));

                // normal belt in that direction
                let new_history =
                    walker.history.iter().copied().chain(iter::once(Belt::Normal(dir))).collect();
                walkers.push(Mazewalker { pos: goto, history: new_history });
            }

            // underground belts in the direction the last belt is pointing
            if opts.contains(RoutingOptimizations::MYLEE_USE_UNDERGROUND_BELTS) {
                let dir = match walker.history.last() {
                    Some(belt) => belt.direction(),
                    None => continue,
                };
                for gap in 0..=4 {
                    let underground_end = walker.pos + (dir.to_vector() * (gap + 1));
                    // check for no interference with other underground belts in the way
                    match pcb.entity_at(underground_end) {
                        Some(Entity { function: Function::UndergroundBelt(intersecting_dir, _), .. }) if intersecting_dir.is_same_axis(dir) => break,
                        Some(_) => continue,
                        _ => (),
                    }
                    // we can't land directly on the field we want to reach with an underground belt
                    if underground_end == to || visited_field_directions.contains(&(underground_end, dir)) || !bounds.contains(underground_end) {
                        continue;
                    }

                    let goto = underground_end + dir.to_vector();
                    if visited_field_directions.contains(&(goto, dir)) || !bounds.contains(goto) || pcb.is_blocked(goto) {
                        continue;
                    }

                    visited_fields.insert(goto.clone());
                    visited_field_directions.insert((goto, dir));
                    let new_history = walker.history.iter().copied().chain(iter::once(Belt::Underground { dir, gap })).collect();
                    if goto == to {
                        let mut path = walker.history;
                        path.push(Belt::Normal(dir));
                        return Some(path);
                    }
                    walkers.push(Mazewalker { pos: goto, history: new_history });
                }
            }
        }
    }
    None
}

