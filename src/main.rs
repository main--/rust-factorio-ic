use factorio_ic::run;
use factorio_ic::pcb::GridPcb;
use factorio_ic::routing::{self, MyleeOptions};

fn main() {
    //run("automation-science-pack", 0.2, |pcb: &mut GridPcb, f, t| routing::lee_pathfinder(pcb, f, t));
    run("chemical-science-pack", 0.3, |pcb: &mut GridPcb, f, t| routing::mylee(pcb, f, t, MyleeOptions::USE_UNDERGROUND_BELTS | MyleeOptions::VISITED_WITH_DIRECTIONS));
}

