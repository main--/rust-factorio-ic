use leemaze::{maze_directions2d, AllowedMoves2D};
use fehler::throws;

use crate::pcb::{Pcb, Direction, Point, ALL_DIRECTIONS, NeededWire};
use crate::routing::{apply_lee_path, insert_underground_belts};

#[throws(())]
pub fn lee_pathfinder(pcb: &mut impl Pcb, &NeededWire { from, to, wire_kind }: &NeededWire) {
    let lee_rect = pcb.entity_rect().pad(2);

    let mut rows = Vec::new();
    for y in lee_rect.a.y..lee_rect.b.y {
        let mut row = Vec::new();
        for x in lee_rect.a.x..lee_rect.b.x {
            row.push(Point::new(x, y) != to && pcb.is_blocked(Point::new(x, y)));
        }
        rows.push(row);
    }

    let lee_from = from - lee_rect.a.coords;
    let lee_to = to - lee_rect.a.coords;

//    println!("{}", render::ascii_wire_to_route(&rows, lee_from, lee_to));

    let lee_to = (lee_to.x as usize, lee_to.y as usize);
    let lee_from = (lee_from.x as usize, lee_from.y as usize);

    let moveset = AllowedMoves2D {
        moves: ALL_DIRECTIONS.iter().map(Direction::to_vector).map(|v| (v.x, v.y)).collect(),
    };
    let path = maze_directions2d(&rows, &moveset, &lee_from, &lee_to).ok_or(())?;

//    println!("{}", render::ascii_routed_wire(&rows, &path2));
    let path = path.into_iter().map(|i| ALL_DIRECTIONS[i]);
    let path = insert_underground_belts(path, wire_kind.gap_size());
    apply_lee_path(pcb, Point::new(from.x, from.y), path, wire_kind)
}

