use nalgebra::geometry::{Point2, Translation2};

use crate::{Entity, Direction, Function};

type Point = Point2<i32>;
type Translation = Translation2<i32>;

// # Types of wires:
//
// One-to-many: One gears assembler feeds many automation science pack assemblers
// Trivial implementation: Belt connection
// Many-to-one
// Trivial implementation: Belt connection
// Lane merge
// Trivial implementation: L+R construction

fn lee_pathfinder_new(entities: &mut Vec<Entity>, from: (i32, i32), to: (i32, i32)) {
    let moveset = [
        (Direction::Right, Translation::new(1, 0)),
        (Direction::Down, Translation::new(0, 1)),
        (Direction::Left, Translation::new(-1, 0)),
        (Direction::Up, Translation::new(0, -1)),
    ];

    let from = Point2::new(from.0, from.1);
    let to = Point2::new(to.0, to.1);

    let path = mylee(entities, &moveset, from, to);

    let mut cursor = from;
    for step in path.unwrap() {
        let mov = moveset[step];

        entities.retain(|e| !e.overlaps(cursor.x, cursor.y)); // delete conflicting entities
        entities.push(Entity { x: cursor.x, y: cursor.y, function: Function::Belt(mov.0) });

        cursor = mov.1.transform_point(&cursor);
    }
}


fn mylee(
    entities: &[Entity], moveset: &[(Direction, Translation)], from: Point, to: Point,
) -> Option<Vec<usize>> {
    struct Mazewalker {
        pos: Point,
        history: Vec<usize>,
    }

    let mut blocked_coords = Vec::new();

    // let from = Point2::new(from.0, from.1);
    // let to = Point2::new(to.0, to.1);

    // TODO: there's probably a much better algorithm based around some kind of cost heuristic
    let mut walkers = vec![Mazewalker { pos: from, history: Vec::new() }];
    while !walkers.is_empty() {
        println!("{} walkers {} blockers", walkers.len(), blocked_coords.len());
        for walker in std::mem::replace(&mut walkers, Vec::new()) {
            println!("{} vs {}", walker.pos, to);
            if walker.pos == to {
                return Some(walker.history);
            }

            for (i, &(_, trans)) in moveset.iter().enumerate() {
                let goto = trans.transform_point(&walker.pos);
                if entities.iter().any(|e| e.overlaps(goto.x, goto.y)) {
                    // blocked with existing entity
                    continue;
                }
                if blocked_coords.contains(&goto) {
                    // blocked with temporary entity
                    continue;
                }
                if goto.x.abs() > 30 || goto.y.abs() > 30 {
                    continue;
                }

                blocked_coords.push(goto); // could be a hashset

                let new_history =
                    walker.history.iter().copied().chain(std::iter::once(i)).collect();
                walkers.push(Mazewalker { pos: goto, history: new_history });
            }
        }
    }
    None
}

