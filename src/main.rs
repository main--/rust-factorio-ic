use std::env;

use crate::pcb::{Entity, Function, Direction, Pcb, NeededWires, Point};
use crate::routing::RoutingOptimizations;

mod recipe;
mod kirkmcdonald;
mod pcb;
mod placement;
mod routing;
mod render;

fn main() {
    let path = env::args().nth(1).unwrap_or(
        "/home/morpheus/.steam/steam/steamapps/common/Factorio/data/base/prototypes/recipe".to_string()
    );
    let recipes = recipe::extract_recipes(path).unwrap();
    println!("Parsed {} recipes", recipes.len());

    let tree = kirkmcdonald::kirkmcdonald(&recipes, "chemical-science-pack", 0.1);
    println!("{:#?}", tree);
    let needed_assemblers: Vec<_> = kirkmcdonald::needed_assemblers(&tree).collect();
    println!("assemblers needed: {:?}", needed_assemblers);

    // very simple and stupid grid placer
    let gridsize = (needed_assemblers.len() as f64).sqrt().ceil() as i32;
    println!("gridsize={}", gridsize);

    let mut grid_i = 0;
    let mut pcb = Pcb::new();
    let mut needed_wires = NeededWires::new();
    let (lins, lout) =
        placement::simple_grid(&tree, &mut grid_i, &mut pcb, &mut needed_wires, gridsize).unwrap();

    let gap_upper = 3;
    pcb.add_all(&[
        Entity { location: Point::new(0, -3 - gap_upper), function: Function::Belt(Direction::Up) },
        Entity { location: Point::new(0, -4 - gap_upper), function: Function::Belt(Direction::Up) },
    ]);
    for i in 0..lins.len() {
        pcb.add(Entity {
            location: Point::new(i as i32 + 1, -3 - gap_upper),
            function: Function::Belt(Direction::Down),
        });
        pcb.add(Entity {
            location: Point::new(i as i32 + 1, -4 - gap_upper),
            function: Function::Belt(Direction::Down),
        });
    }
    needed_wires.push((lout, (0, -3 - gap_upper)));
    for (i, lin) in lins.into_iter().enumerate().rev() {
        needed_wires.push(((i as i32 + 1, -3 - gap_upper), lin));
    }

    println!("rendering {} wires", needed_wires.len());

    // routing::route(&mut pcb, &mut needed_wires, routing::lee_pathfinder, RoutingOptimizations::empty());
    routing::route(&mut pcb, &mut needed_wires, routing::mylee, RoutingOptimizations::MYLEE_PREFER_SAME_DIRECTION);

    println!("{}", render::blueprint(&pcb));
    println!("{}", render::ascii(&pcb));
}
