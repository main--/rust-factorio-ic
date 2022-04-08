use factorio_ic::run;
use factorio_ic::pcb::GridPcb;
use factorio_ic::routing::{self, MyleeOptions};

fn main() {
    //run("automation-science-pack", 100., |pcb: &mut GridPcb, f, t| routing::lee_pathfinder(pcb, f, t));
    //run("chemical-science-pack", 0.3, |pcb: &mut GridPcb, f, t| routing::mylee(pcb, f, t, MyleeOptions::USE_UNDERGROUND_BELTS | MyleeOptions::VISITED_WITH_DIRECTIONS));
    //run("chemical-science-pack", 1., |pcb: &mut GridPcb, f, t| routing::mylee(pcb, f, t, MyleeOptions::USE_UNDERGROUND_BELTS | MyleeOptions::VISITED_WITH_DIRECTIONS));
    run("utility-science-pack", 0.6, |pcb: &mut GridPcb, w| routing::mylee(pcb, w, MyleeOptions::USE_UNDERGROUND_BELTS | MyleeOptions::VISITED_WITH_DIRECTIONS));
}

