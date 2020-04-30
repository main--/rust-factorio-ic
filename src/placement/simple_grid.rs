use crate::{Entity, Direction, Function};
use crate::kirkmcdonald::ProductionGraph;
use crate::pcb::Pcb;

pub fn gridrender_subtree(
    subtree: &ProductionGraph, grid_i: &mut i32, pcb: &mut Pcb,
    needed_wires: &mut Vec<((i32, i32), (i32, i32))>, gridsize: i32,
) -> Option<(Vec<(i32, i32)>, (i32, i32))> {
    if subtree.building == "assembler" {
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

            pcb.entities_mut().extend(vec![
                Entity {
                    x: startx + 2,
                    y: starty + 0,
                    function: Function::Assembler { recipe: subtree.output.clone() },
                },
                // output belt
                Entity { x: startx + 0, y: starty + 0, function: Function::Belt(Direction::Down) },
                Entity { x: startx + 0, y: starty + 1, function: Function::Belt(Direction::Down) },
                Entity { x: startx + 0, y: starty + 2, function: Function::Belt(Direction::Down) },
                Entity {
                    x: startx + 1,
                    y: starty + 1,
                    function: Function::Inserter {
                        orientation: Direction::Left,
                        long_handed: false,
                    },
                },
                // input belt
                Entity { x: startx + 6, y: starty + 0, function: Function::Belt(Direction::Up) },
                Entity { x: startx + 6, y: starty + 1, function: Function::Belt(Direction::Up) },
                Entity { x: startx + 6, y: starty + 2, function: Function::Belt(Direction::Up) },
                Entity {
                    x: startx + 5,
                    y: starty + 0,
                    function: Function::Inserter {
                        orientation: Direction::Left,
                        long_handed: false,
                    },
                },
            ]);
            if let Some((sx, sy)) = prev {
                needed_wires.push(((sx + 0, sy + 2), (startx + 0, starty + 0)));
                needed_wires.push(((startx + 6, starty + 0), (sx + 6, sy + 2)));
            }

            if second_input_belt {
                pcb.entities_mut().extend(vec![
                    // input belt 2
                    Entity {
                        x: startx + 7,
                        y: starty + 0,
                        function: Function::Belt(Direction::Up),
                    },
                    Entity {
                        x: startx + 7,
                        y: starty + 1,
                        function: Function::Belt(Direction::Up),
                    },
                    Entity {
                        x: startx + 7,
                        y: starty + 2,
                        function: Function::Belt(Direction::Up),
                    },
                    Entity {
                        x: startx + 5,
                        y: starty + 1,
                        function: Function::Inserter {
                            orientation: Direction::Left,
                            long_handed: true,
                        },
                    },
                ]);
                if let Some((sx, sy)) = prev {
                    needed_wires.push(((startx + 7, starty + 0), (sx + 7, sy + 2)));
                }
            }

            prev = Some((startx, starty));
            *grid_i += 1;
        }

        let (sx, sy) = prev.unwrap();
        let my_output = (sx + 0, sy + 2);
        // connect intra here
        let mut target_points = Vec::new();
        if our_inputs.len() == 1 {
            // single input, so no lane organization needed
            target_points.push((sx + 6, sy + 2));
        } else {
            pcb.entities_mut().extend(vec![
                Entity { x: sx + 6, y: sy + 3, function: Function::Belt(Direction::Up) },
                Entity { x: sx + 5, y: sy + 3, function: Function::Belt(Direction::Right) },
                Entity { x: sx + 7, y: sy + 3, function: Function::Belt(Direction::Left) },
            ]);
            target_points.push((sx + 5, sy + 3));
            target_points.push((sx + 7, sy + 3));

            if second_input_belt {
                if our_inputs.len() == 3 {
                    target_points.push((sx + 7, sy + 2));
                } else {
                    pcb.entities_mut().extend(vec![
                        Entity { x: sx + 8, y: sy + 2, function: Function::Belt(Direction::Left) },
                        Entity { x: sx + 8, y: sy + 1, function: Function::Belt(Direction::Down) },
                        Entity { x: sx + 8, y: sy + 3, function: Function::Belt(Direction::Up) },
                    ]);
                    target_points.push((sx + 8, sy + 2));
                    target_points.push((sx + 8, sy + 3));
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
