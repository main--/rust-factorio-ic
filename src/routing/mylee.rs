use std::collections::HashSet;

use fehler::throws;
use nalgebra::geometry::Point2;
use nalgebra::base::Vector2;

use crate::pcb::{Entity, Direction, Function, Pcb};
use crate::render;

type Point = Point2<i32>;
type Vector = Vector2<i32>;

struct Mazewalker {
    pos: Point,
    history: Vec<usize>,
}

// # Types of wires:
//
// One-to-many: One gears assembler feeds many automation science pack assemblers
// Trivial implementation: Belt connection
// Many-to-one
// Trivial implementation: Belt connection
// Lane merge
// Trivial implementation: L+R construction

#[throws(())]
pub fn lee_pathfinder_new(pcb: &mut Pcb, from: (i32, i32), to: (i32, i32)) {
    let moveset = [
        (Direction::Right, Vector::new(1, 0)),
        (Direction::Down, Vector::new(0, 1)),
        (Direction::Left, Vector::new(-1, 0)),
        (Direction::Up, Vector::new(0, -1)),
    ];

    let from = Point2::new(from.0, from.1);
    let to = Point2::new(to.0, to.1);

    println!("{}", render::ascii(pcb));
    println!("from: {:?}, to: {:?}", from, to);
    let path = mylee(pcb, &moveset, from, to);

    let mut cursor = from;
    for step in path.ok_or(())? {
        let mov = moveset[step];

        pcb.replace(Entity { x: cursor.x, y: cursor.y, function: Function::Belt(mov.0) });

        cursor += mov.1;
    }
}


fn mylee(
    pcb: &Pcb, moveset: &[(Direction, Vector)], from: Point, to: Point,
) -> Option<Vec<usize>> {

    let mut visited_fields = HashSet::new();

    // TODO: there's probably a much better algorithm based around some kind of cost heuristic
    let mut walkers = vec![Mazewalker { pos: from, history: Vec::new() }];

    while !walkers.is_empty() {
//        println!("{} walkers {} visited", walkers.len(), visited_fields.len());



        for walker in std::mem::replace(&mut walkers, Vec::new()) {
//            println!("{} vs {}", walker.pos, to);
            for (i, &(_, trans)) in moveset.iter().enumerate() {
                let goto = walker.pos + trans;
                if goto == to {
                    let mut walker = walker;
                    walker.history.push(i);
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
                if goto.x.abs() > 100 || goto.y.abs() > 100 {
                    continue;
                }

                visited_fields.insert(goto.clone());

                let new_history =
                    walker.history.iter().copied().chain(std::iter::once(i)).collect();
                walkers.push(Mazewalker { pos: goto, history: new_history });
            }
        }
    }
    None
}

