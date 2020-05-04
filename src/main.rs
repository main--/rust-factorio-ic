use std::env;

use crate::pcb::{Entity, Function, Direction};
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

    //let tree = kirkmcdonald::kirkmcdonald(&recipes, "automation-science-pack", 0.1);
    //let tree = kirkmcdonald::kirkmcdonald(&recipes, "logistic-science-pack", 1.);
    let tree = kirkmcdonald::kirkmcdonald(&recipes, "chemical-science-pack", 0.3);
    println!("{:#?}", tree);

    let mut pcb = pcb::GridPcb::default();
    let needed_wires = placement::simple_grid(&mut pcb, &tree);

    println!("rendering {} wires", needed_wires.len());

    //routing::route(&mut pcb, needed_wires, |pcb, f, t| routing::mylee(pcb, f, t, RoutingOptimizations::empty()));
    routing::route(&mut pcb, needed_wires, |pcb, f, t| routing::mylee(pcb, f, t, RoutingOptimizations::MYLEE_USE_UNDERGROUND_BELTS | RoutingOptimizations::MYLEE_VISITED_WITH_DIRECTIONS));

    println!("{}", render::blueprint(&pcb));
    println!("{}", render::ascii(&pcb));
}
