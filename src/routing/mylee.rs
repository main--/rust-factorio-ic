use std::collections::HashSet;

use fehler::throws;

use crate::pcb::{Direction, Pcb, Point, Vector, ALL_DIRECTIONS};
use crate::render;
use crate::routing::apply_lee_path;

// # Types of wires:
//
// One-to-many: One gears assembler feeds many automation science pack assemblers
// Trivial implementation: Belt connection
// Many-to-one
// Trivial implementation: Belt connection
// Lane merge
// Trivial implementation: L+R construction

#[throws(())]
pub fn mylee(pcb: &mut Pcb, from: (i32, i32), to: (i32, i32)) {
    let from = Point::new(from.0, from.1);
    let to = Point::new(to.0, to.1);

    let path = mylee_internal(pcb, &ALL_DIRECTIONS, from, to).ok_or(())?;

    apply_lee_path(pcb, from, path);
}

struct Mazewalker {
    pos: Point,
    history: Vec<Direction>,
}

fn mylee_internal(
    pcb: &Pcb, moveset: &[Direction], from: Point, to: Point,
) -> Option<Vec<Direction>> {
    // ensure enough space around possible entities to possibly lay a belt around everything,
    // including a possible underground belt out, followed by an underground belt back in
    // and the connection loop
    let mut bounds = pcb.entity_rect();
    bounds.a += Vector::new(-2, -2);
    bounds.b += Vector::new(2, 2);

    let mut visited_fields = HashSet::new();

    // TODO: there's probably a much better algorithm based around some kind of cost heuristic
    let mut walkers = vec![Mazewalker { pos: from, history: Vec::new() }];

    while !walkers.is_empty() {
//        println!("{} walkers {} visited", walkers.len(), visited_fields.len());

        for walker in std::mem::replace(&mut walkers, Vec::new()) {
//            println!("{} vs {}", walker.pos, to);
            for &dir in moveset.iter() {
                let goto = walker.pos + dir.to_vector();
                if goto == to {
                    let mut walker = walker;
                    walker.history.push(dir);
                    return Some(walker.history);
                }
                if !pcb.is_empty((goto.x, goto.y)) {
                    // blocked with existing entity
                    continue;
                }
                if visited_fields.contains(&goto) {
                    // already visited this field
                    continue;
                }
                if !bounds.contains(goto) {
                    continue;
                }

                visited_fields.insert(goto.clone());

                let new_history =
                    walker.history.iter().copied().chain(std::iter::once(dir)).collect();
                walkers.push(Mazewalker { pos: goto, history: new_history });
            }
        }
    }
    None
}

