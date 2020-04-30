use fehler::throws;

use crate::pcb::{Pcb, Entity, Direction, Function};
use crate::render;

#[throws(())]
pub fn lee_pathfinder(pcb: &mut Pcb, from: (i32, i32), to: (i32, i32)) {
    use leemaze::{maze_directions2d, AllowedMoves2D};

    let max_x = pcb.entities().iter().map(|x| x.x + x.size_x()).max().unwrap_or(0) + 10;
    let max_y = pcb.entities().iter().map(|x| x.y + x.size_y()).max().unwrap_or(0) + 10;

    let mut rows = Vec::new();
    for y in -10..max_y {
        let mut row = Vec::new();
        for x in -10..max_x {
            row.push((x, y) != to && pcb.entities().iter().any(|e| e.overlaps(x, y)));
        }
        rows.push(row);
    }

    println!("{}", render::ascii_wire_to_route(&rows, (from.0 + 10, from.1 + 10), (to.0 + 10, to.1 + 10)));

    // for row in &rows {
    // for &x in row {
    // if x {
    // print!("X");
    // } else {
    // print!(" ");
    // }
    // }
    // println!();
    // }

    let moveset = AllowedMoves2D {
        moves: vec![
            (-1, 0),
            (1, 0),
            (0, 1),
            (0, -1),
            /*            // underground belts
             * (6, 0),
             * (0, 6),
             * (-6, 0),
             * (0, -6), */
        ],
    };
    let path = maze_directions2d(
        &rows,
        &moveset,
        &((from.0 + 10) as usize, (from.1 + 10) as usize),
        &((to.0 + 10) as usize, (to.1 + 10) as usize),
    );
    //    println!("{:?}", path);

    let moveset_dir = [Direction::Left, Direction::Right, Direction::Down, Direction::Up];

    let mut path2 = vec![(from.0 + 10, from.1 + 10)];
    let path = path.ok_or(())?;
    for &step in &path {
        let prev = path2.last().unwrap();
        let mov = moveset.moves[step];
        let next = (prev.0 + mov.0, prev.1 + mov.1);
        path2.push(next);
    }
    //    println!("{:?}", path2);

    println!("{}", render::ascii_routed_wire(&rows, &path2));

    let mut undergrounded_path = Vec::new();
    let mut cut_iter = path.iter();
    while let Some(&current_direction) = cut_iter.next() {
        let is_continuation = match undergrounded_path.last() {
            Some(Ok(cd)) if *cd == current_direction => true,
            Some(Err((cd, gap))) if *cd == current_direction => true,
            _ => false,
        };
        let mut tail_length = cut_iter.clone().take_while(|&&d| d == current_direction).count();
        if is_continuation {
            tail_length += 1;
        }
        if tail_length > 2 {
            let gap = std::cmp::min(tail_length - 2, 4) as i32;

            for _ in 0..(gap + 1) {
                cut_iter.next().unwrap();
            }

            if !is_continuation {
                cut_iter.next().unwrap();
                undergrounded_path.push(Ok(current_direction)); // landing pad
            }
            undergrounded_path.push(Err((current_direction, gap))); // actual underground
        } else {
            undergrounded_path.push(Ok(current_direction));
        }
    }
    let mut cursor = from;
    for step in undergrounded_path {
        let (x, y) = cursor;
        pcb.entities_mut().retain(|e| !e.overlaps(x, y)); // delete conflicting entities

        match step {
            Ok(step) => {
                pcb.entities_mut().push(Entity { x, y, function: Function::Belt(moveset_dir[step]) });

                let mov = moveset.moves[step];
                cursor = (x + mov.0, y + mov.1);
            },
            Err((step, gap)) => {
                pcb.entities_mut().push(Entity {
                    x,
                    y,
                    function: Function::UndergroundBelt(moveset_dir[step], true),
                });
                let mov = moveset.moves[step];
                pcb.entities_mut().push(Entity {
                    x: x + mov.0 * (gap + 1),
                    y: y + mov.1 * (gap + 1),
                    function: Function::UndergroundBelt(moveset_dir[step], false),
                });

                cursor = (x + mov.0 * (gap + 2), y + mov.1 * (gap + 2));
            },
        }
    }
    // let mut cut_iter = 0;
    // while cut_iter < path.len() {
    // let current_direction = path[cut_iter];
    // let run_length = path[cut_iter..].iter().take_while(|&&d| d == current_direction).count();
    // if run_length > 3 {
    // let gap = std::cmp::min(run_length - 3, 4);
    // let gap_start = cut_iter + 2;
    // path.drain(gap_start .. (gap_start + gap));
    //
    // cut_iter += 3;
    //
    // for _ in 0..gap {
    // path.insert(gap_start, current_direction + 4);
    // cut_iter += 1;
    // }
    // } else {
    // cut_iter += 1;
    // }
    // }
    //
    // let mut cursor = from;
    // for &step in &path {
    // let (x, y) = cursor;
    // if step >= 4 {
    // let mov = moveset.moves[step - 4];
    // cursor = (x + mov.0, y + mov.1);
    // continue;
    // }
    //
    // entities.retain(|e| !e.overlaps(x, y)); // delete conflicting entities
    // entities.push(Entity { x, y, function: Function::Belt(moveset_dir[step]) });
    //
    // let mov = moveset.moves[step];
    // cursor = (x + mov.0, y + mov.1);
    // }
}
