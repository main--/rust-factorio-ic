use std::env;

use num_rational::Rational32;
use pcb::NeededWire;

use crate::pcb::{Pcb, Entity, Function, Direction};
use crate::placement::{Placer, BusPlacer};

mod recipe;
mod kirkmcdonald;
pub mod pcb;
mod placement;
pub mod routing;
mod render;

type Rational = Rational32;

pub fn run<P: Pcb>(recipe: &str, amount: f64, pathfinder: impl Fn(&mut P, &NeededWire) -> Result<(), ()>) {
    let path = env::args().nth(1).unwrap_or(
        "recipe".to_string()
    );
    let recipes = recipe::extract_recipes(path).unwrap();
    println!("Parsed {} recipes", recipes.len());

    let desired_per_second = Rational::approximate_float(amount).unwrap();
    let tree = kirkmcdonald::kirkmcdonald(&recipes, recipe, desired_per_second, &pcb::WireKind::Belt);
    println!("{:#?}", tree);

    let mut pcb = P::default();
    let needed_wires = BusPlacer::place(&mut pcb, &tree);

    println!("rendering {} wires", needed_wires.len());

    //routing::route(&mut pcb, needed_wires, |pcb, w| routing::mylee(pcb, w, MyleeOptimizations::empty()));
    routing::route(&mut pcb, needed_wires, pathfinder);

    println!("{}", render::blueprint(&pcb));
    println!("{}", render::ascii(&pcb));
}

#[cfg(test)]
mod test {
    use super::pcb::{Pcb, GridPcb, HashmapPcb as HashPcb};
    use super::routing::{self, MyleeOptions};


    #[cfg(feature = "leemaze_lib")]
    fn run_leemaze<P: Pcb>(recipe: &str, amount: f64) { super::run(recipe, amount, |pcb: &mut P, w| routing::lee_pathfinder(pcb, w)); }
    fn run_mylee_bad<P: Pcb>(recipe: &str, amount: f64) { super::run(recipe, amount, |pcb: &mut P, w| routing::mylee(pcb, w, MyleeOptions::empty())); }
    fn run_mylee_bad_preferdir<P: Pcb>(recipe: &str, amount: f64) { super::run(recipe, amount, |pcb: &mut P, w| routing::mylee(pcb, w, MyleeOptions::PREFER_SAME_DIRECTION)); }
    fn run_mylee_underground_bad<P: Pcb>(recipe: &str, amount: f64) { super::run(recipe, amount, |pcb: &mut P, w| routing::mylee(pcb, w, MyleeOptions::USE_UNDERGROUND_BELTS)); }
    fn run_good<P: Pcb>(recipe: &str, amount: f64) { super::run(recipe, amount, |pcb: &mut P, w| routing::mylee(pcb, w, MyleeOptions::USE_UNDERGROUND_BELTS | MyleeOptions::VISITED_WITH_DIRECTIONS)); }
    fn run_mylee_underground_preferdir<P: Pcb>(recipe: &str, amount: f64) { super::run(recipe, amount, |pcb: &mut P, w| routing::mylee(pcb, w, MyleeOptions::USE_UNDERGROUND_BELTS | MyleeOptions::VISITED_WITH_DIRECTIONS | MyleeOptions::PREFER_SAME_DIRECTION)); }

    #[test] fn automation_0_75_grid() { run_good::<GridPcb>("automation-science-pack", 0.75) }
    #[test] fn automation_0_75_hash() { run_good::<HashPcb>("automation-science-pack", 0.75) }
    #[test] fn automation_5_00_grid() { run_good::<GridPcb>("automation-science-pack", 5.00) }
    #[test] fn automation_5_00_hash() { run_good::<HashPcb>("automation-science-pack", 5.00) }
    #[test] #[cfg(feature = "leemaze_lib")] fn automation_0_75_leegacy() { run_leemaze::<GridPcb>("automation-science-pack", 0.75) }

    #[test] fn logistic_0_75_grid() { run_good::<GridPcb>("logistic-science-pack", 0.75) }
    #[test] fn logistic_0_75_hash() { run_good::<HashPcb>("logistic-science-pack", 0.75) }
    #[test] #[cfg(feature = "leemaze_lib")] fn logistic_0_75_leegacy() { run_leemaze::<GridPcb>("logistic-science-pack", 0.75) }
    #[test] fn logistic_0_75_mylee_bad() { run_mylee_bad::<GridPcb>("logistic-science-pack", 0.75) }
    #[test] fn logistic_0_75_mylee_bad_preferdir() { run_mylee_bad_preferdir::<GridPcb>("logistic-science-pack", 0.75) }
    //#[test] fn logistic_0_75_mylee_underground_bad() { run_mylee_underground_bad::<GridPcb>("logistic-science-pack", 0.75) }
    #[test] fn logistic_0_75_mylee_underground_preferdir() { run_mylee_underground_preferdir::<GridPcb>("logistic-science-pack", 0.75) }

    #[test] fn chemical_0_10_grid() { run_good::<GridPcb>("chemical-science-pack", 0.10) }
    #[test] fn chemical_0_10_hash() { run_good::<HashPcb>("chemical-science-pack", 0.10) }
    #[test] fn chemical_0_10_underground_bad() { run_mylee_underground_bad::<GridPcb>("chemical-science-pack", 0.10) } // issue #14

    #[test] fn utility_0_10_grid() { run_good::<GridPcb>("utility-science-pack", 0.10) }
    #[test] fn production_0_10_grid() { run_good::<GridPcb>("production-science-pack", 0.10) }
}

