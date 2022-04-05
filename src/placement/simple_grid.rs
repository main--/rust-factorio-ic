//! very simple and stupid grid placer

use crate::{Entity, Direction, Function};
use crate::kirkmcdonald::ProductionGraph;
use crate::pcb::{Pcb, Point, Vector, NeededWires, need_belt};
use crate::recipe::Category;
use super::Placer;

use std::iter;

pub struct SimpleGridPlacer;

impl Placer for SimpleGridPlacer {
    fn place(pcb: &mut impl Pcb, tree: &ProductionGraph) -> NeededWires { simple_grid(pcb, tree) }
}

fn simple_grid(pcb: &mut impl Pcb, tree: &ProductionGraph) -> NeededWires {
    let needed_assemblers = needed_cells(&tree).count();

    let gridsize = (needed_assemblers as f64).sqrt().ceil() as i32;
    println!("gridsize={}", gridsize);

    let mut grid_i = 0;
    let mut needed_wires = NeededWires::new();
    let (lins, lout) = gridrender_subtree(&tree, &mut grid_i, pcb, &mut needed_wires, gridsize).unwrap();

    let gap_upper = 10;
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
    needed_wires.push(need_belt(lout, Point::new(0, -3 - gap_upper)));
    for (i, lin) in lins.into_iter().enumerate().rev() {
        needed_wires.push(need_belt(Point::new(i as i32 + 1, -3 - gap_upper), lin));
    }
    needed_wires
}

fn needed_cells<'a>(g: &'a ProductionGraph) -> Box<dyn Iterator<Item = &'a str> + 'a> {
    let upstream = g.inputs.iter().flat_map(needed_cells);
    if g.building == Some(Category::Assembler) || g.building == Some(Category::Furnace) {
        println!("i={}", g.inputs.len());
        Box::new(iter::repeat(&g.output as &str).take(g.how_many.ceil().to_integer() as usize).chain(upstream))
    } else {
        Box::new(upstream)
    }
}

fn gridrender_subtree(
    subtree: &ProductionGraph, grid_i: &mut i32, pcb: &mut impl Pcb,
    needed_wires: &mut NeededWires, gridsize: i32,
) -> Option<(Vec<Point>, Point)> {
    if subtree.building == Some(Category::Assembler) || subtree.building == Some(Category::Furnace) {
        let mut upper_inputs = Vec::new();
        let mut our_inputs = Vec::new();

        for input in &subtree.inputs {
            match gridrender_subtree(input, grid_i, pcb, needed_wires, gridsize) {
                None => {
                    // becomes an input instead
                    our_inputs.push(None);
                },
                Some((ui, out)) => {
                    upper_inputs.extend(ui);
                    our_inputs.push(Some(out));
                },
            }
        }

        assert_eq!(subtree.inputs.len(), our_inputs.len());
        let second_input_belt = match subtree.inputs.len() {
            1 | 2 => false,
            3 | 4 => true,
            _ => unreachable!(),
        };

        let howmany = subtree.how_many.ceil().to_integer() as usize;
        let mut prev = None;
        for _ in 0..howmany {
            let i = *grid_i;
            let grid_x = i % gridsize;
            let grid_y = i / gridsize;

            let cell_size_x = 15;
            let cell_size_y = 9;

            let start = Point::new(cell_size_x * grid_x, cell_size_y * grid_y);

            let main_function = match subtree.building {
                Some(Category::Assembler) => Function::Assembler { recipe: subtree.output.clone() },
                Some(Category::Furnace) => Function::Furnace,
                _ => unreachable!(),
            };

            pcb.add_all(&[
                Entity {
                    location: start + Vector::new(2, 0),
                    function: main_function,
                },
                // output belt
                Entity { location: start + Vector::new(0, 0), function: Function::Belt(Direction::Down) },
                Entity { location: start + Vector::new(0, 1), function: Function::Belt(Direction::Down) },
                Entity { location: start + Vector::new(0, 2), function: Function::Belt(Direction::Down) },
                Entity {
                    location: start + Vector::new(1, 1),
                    function: Function::Inserter {
                        orientation: Direction::Left,
                        long_handed: false,
                    },
                },
                // input belt
                Entity { location: start + Vector::new(6, 0), function: Function::Belt(Direction::Left) },
                Entity { location: start + Vector::new(6, 1), function: Function::Belt(Direction::Up) },
                Entity { location: start + Vector::new(6, 2), function: Function::Belt(Direction::Up) },
                Entity {
                    location: start + Vector::new(5, 0),
                    function: Function::Inserter {
                        orientation: Direction::Left,
                        long_handed: false,
                    },
                },
                Entity { location: start + Vector::new(3, 3), function: Function::ElectricPole },
            ]);

            if (grid_y == 0) && (grid_x != (gridsize - 1)) {
                pcb.add(Entity { location: start + Vector::new(10, 1), function: Function::ElectricPole });
            }

            if let Some(prev) = prev {
                needed_wires.push(need_belt(prev + Vector::new(0, 2), start + Vector::new(0, 0)));
                needed_wires.push(need_belt(start + Vector::new(6, 0), prev + Vector::new(6, 2)));
            }

            if second_input_belt {
                pcb.add_all(&[
                    // input belt 2
                    Entity {
                        location: start + Vector::new(7, 0),
                        function: Function::Belt(Direction::Down),
                    },
                    Entity {
                        location: start + Vector::new(7, 1),
                        function: Function::Belt(Direction::Up),
                    },
                    Entity {
                        location: start + Vector::new(7, 2),
                        function: Function::Belt(Direction::Up),
                    },
                    Entity {
                        location: start + Vector::new(5, 1),
                        function: Function::Inserter {
                            orientation: Direction::Left,
                            long_handed: true,
                        },
                    },
                ]);
                if let Some(prev) = prev {
                    needed_wires.push(need_belt(start + Vector::new(7, 0), prev + Vector::new(7, 2)));
                }
            }

            prev = Some(start);
            *grid_i += 1;
        }

        let prev = prev.unwrap();
        let my_output = prev + Vector::new(0, 2);
        // connect intra here
        let mut target_points = Vec::new();
        if our_inputs.len() == 1 {
            // single input, so no lane organization needed
            target_points.push(prev + Vector::new(6, 2));
        } else {
            pcb.add_all(&[
                Entity { location: prev + Vector::new(6, 3), function: Function::Belt(Direction::Up) },
                Entity { location: prev + Vector::new(5, 3), function: Function::Belt(Direction::Right) },
                Entity { location: prev + Vector::new(7, 3), function: Function::Belt(Direction::Left) },
            ]);
            target_points.push(prev + Vector::new(5, 3));
            target_points.push(prev + Vector::new(7, 3));

            if second_input_belt {
                if our_inputs.len() == 3 {
                    target_points.push(prev + Vector::new(7, 2));
                } else {
                    pcb.add_all(&[
                        Entity { location: prev + Vector::new(8, 2), function: Function::Belt(Direction::Left) },
                        Entity { location: prev + Vector::new(8, 1), function: Function::Belt(Direction::Down) },
                        Entity { location: prev + Vector::new(8, 3), function: Function::Belt(Direction::Up) },
                    ]);
                    target_points.push(prev + Vector::new(8, 2));
                    target_points.push(prev + Vector::new(8, 3));
                }
            }
        }

        assert_eq!(our_inputs.len(), target_points.len());
        for (from, to) in our_inputs.into_iter().zip(target_points) {
            match from {
                None => upper_inputs.push(to),
                Some(from) => needed_wires.push(need_belt(from, to)),
            }
        }

        Some((upper_inputs, my_output))
    } else {
        None
    }
}
