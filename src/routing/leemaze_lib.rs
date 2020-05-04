use fehler::throws;

use crate::pcb::{Pcb, Direction, Point, ALL_DIRECTIONS};
use crate::render;
use crate::routing::{apply_lee_path, RoutingOptimizations, insert_underground_belts};

#[throws(())]
pub fn lee_pathfinder(pcb: &mut impl Pcb, from: Point, to: Point) {
    use leemaze::{maze_directions2d, AllowedMoves2D};

    let max_x = pcb.entities().map(|x| x.location.x + x.size_x()).max().unwrap_or(0) + 10;
    let max_y = pcb.entities().map(|x| x.location.y + x.size_y()).max().unwrap_or(0) + 10;

    let mut rows = Vec::new();
    for y in -10..max_y {
        let mut row = Vec::new();
        for x in -10..max_x {
            row.push(Point::new(x, y) != to && pcb.entities().any(|e| e.overlaps(Point::new(x, y))));
        }
        rows.push(row);
    }

//    println!("{}", render::ascii_wire_to_route(&rows, (from.0 + 10, from.1 + 10), (to.0 + 10, to.1 + 10)));

    let moveset = AllowedMoves2D {
        moves: ALL_DIRECTIONS.iter().map(Direction::to_vector).map(|v| (v.x, v.y)).collect(),
    };
    let path = maze_directions2d(
        &rows,
        &moveset,
        &((from.x + 10) as usize, (from.y + 10) as usize),
        &((to.x + 10) as usize, (to.y + 10) as usize),
    ).ok_or(())?;

//    println!("{}", render::ascii_routed_wire(&rows, &path2));
    let path = path.into_iter().map(|i| ALL_DIRECTIONS[i]);
    let path = insert_underground_belts(path);
    apply_lee_path(pcb, Point::new(from.x, from.y), path)

}
