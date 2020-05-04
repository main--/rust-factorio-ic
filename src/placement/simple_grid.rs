use crate::{Entity, Direction, Function};
use crate::kirkmcdonald::ProductionGraph;
use crate::pcb::{Pcb, Point, Vector, NeededWires};
use crate::recipe::Category;

pub fn gridrender_subtree(
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

        let howmany = subtree.how_many.ceil() as usize;
        let mut prev = None;
        for _ in 0..howmany {
            let i = *grid_i;
            let grid_x = i % gridsize;
            let grid_y = i / gridsize;

            let cell_size_x = 15;
            let cell_size_y = 10;

            let startx = cell_size_x * grid_x;
            let starty = cell_size_y * grid_y;

            let main_function = match subtree.building {
                Some(Category::Assembler) => Function::Assembler { recipe: subtree.output.clone() },
                Some(Category::Furnace) => Function::Furnace,
                _ => unreachable!(),
            };

            pcb.add_all(&[
                Entity {
                    location: Point::new(startx + 2, starty + 0),
                    function: main_function,
                },
                // output belt
                Entity { location: Point::new(startx + 0, starty + 0), function: Function::Belt(Direction::Down) },
                Entity { location: Point::new(startx + 0, starty + 1), function: Function::Belt(Direction::Down) },
                Entity { location: Point::new(startx + 0, starty + 2), function: Function::Belt(Direction::Down) },
                Entity {
                    location: Point::new(startx + 1, starty + 1),
                    function: Function::Inserter {
                        orientation: Direction::Left,
                        long_handed: false,
                    },
                },
                // input belt
                Entity { location: Point::new(startx + 6, starty + 0), function: Function::Belt(Direction::Left) },
                Entity { location: Point::new(startx + 6, starty + 1), function: Function::Belt(Direction::Up) },
                Entity { location: Point::new(startx + 6, starty + 2), function: Function::Belt(Direction::Up) },
                Entity {
                    location: Point::new(startx + 5, starty + 0),
                    function: Function::Inserter {
                        orientation: Direction::Left,
                        long_handed: false,
                    },
                },
            ]);
            if let Some(prev) = prev {
                needed_wires.push((prev + Vector::new(0, 2), Point::new(startx + 0, starty + 0)));
                needed_wires.push((Point::new(startx + 6, starty + 0), prev + Vector::new(6, 2)));
            }

            if second_input_belt {
                pcb.add_all(&[
                    // input belt 2
                    Entity {
                        location: Point::new(startx + 7, starty + 0),
                        function: Function::Belt(Direction::Down),
                    },
                    Entity {
                        location: Point::new(startx + 7, starty + 1),
                        function: Function::Belt(Direction::Up),
                    },
                    Entity {
                        location: Point::new(startx + 7, starty + 2),
                        function: Function::Belt(Direction::Up),
                    },
                    Entity {
                        location: Point::new(startx + 5, starty + 1),
                        function: Function::Inserter {
                            orientation: Direction::Left,
                            long_handed: true,
                        },
                    },
                ]);
                if let Some(prev) = prev {
                    needed_wires.push((Point::new(startx + 7, starty + 0), prev + Vector::new(7, 2)));
                }
            }

            prev = Some(Point::new(startx, starty));
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
                Some(from) => needed_wires.push((from, to)),
            }
        }

        Some((upper_inputs, my_output))
    } else {
        None
    }
}
