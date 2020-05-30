//! bus-style placer

use crate::{Entity, Direction, Function};
use crate::kirkmcdonald::ProductionGraph;
use crate::pcb::{Pcb, Point, Vector, NeededWires};
use crate::recipe::Category;
use crate::render;
use super::Placer;

use std::iter;
use fnv::FnvHashMap;
use petgraph::prelude::*;

pub struct BusPlacer;

static OUTPUT: &'static str = "<output>";

impl Placer for BusPlacer {
    fn place(pcb: &mut impl Pcb, tree: &ProductionGraph) -> NeededWires {
        let mut needed_wires = NeededWires::new();

        // 1. calculate how much we need (i.e. flatten the production graph)
        let mut graph = DiGraphMap::<&str, f64>::new();
        let mut function_map = FnvHashMap::default();
        let mut todo_stack = vec![tree];
        while let Some(item) = todo_stack.pop() {
            if item.building != Some(Category::Assembler) && item.building != Some(Category::Furnace) {
                continue;
            }
        
            todo_stack.extend(&item.inputs);
            
            for input in &item.inputs {
                match graph.add_edge(&input.output, &item.output, input.how_many) {
                    None => (), // all good
                    Some(old) => {
                        // fixup required
                        *graph.edge_weight_mut(&input.output, &item.output).unwrap() += old;
                    }
                }
            }
            
            let function = match item.building {
                Some(Category::Assembler) => Function::Assembler { recipe: item.output.clone() },
                Some(Category::Furnace) => Function::Furnace,
                _ => unreachable!(),
            };
            function_map.insert(&item.output as &str, function);
        }
        println!("{:#?}", graph);
        
        let mut order = petgraph::algo::toposort(&graph, None).expect("there are no cyclic recipes"); // unless you're doing uranium, which is currently excluded
        println!("{:#?}", order);

        graph.add_edge(&tree.output, OUTPUT, tree.how_many);

        let mut global_inputs = Vec::new();        
        for i in (0..order.len()).rev() {
            if graph.neighbors_directed(order[i], petgraph::Direction::Incoming).count() == 0 {
                global_inputs.push(order.remove(i));
            }
        }

        let mut available_outputs = FnvHashMap::<&str, Vec<Point>>::default();

        let gap_upper = -10;
        let mut input_xoffset = 5;
        for input in global_inputs {
            let total_instances_needed = graph.neighbors_directed(input, petgraph::Direction::Outgoing).count() as i32;
            for i in 1..total_instances_needed { // FIXME: this loop is untested, not sure how to trigger it
                for j in 0..(total_instances_needed-2) {
                    pcb.add(Entity { location: Point::new(j, -i) + Vector::new(input_xoffset, gap_upper), function: Function::Belt(Direction::Down) });
                }
                pcb.add(Entity { location: Point::new(total_instances_needed-2, -i) + Vector::new(input_xoffset, gap_upper), function: Function::Splitter(Direction::Down) });
            }
            pcb.add(Entity { location: Point::new(0, -total_instances_needed) + Vector::new(input_xoffset, gap_upper), function: Function::Belt(Direction::Down) });
            pcb.add(Entity { location: Point::new(0, -total_instances_needed - 1) + Vector::new(input_xoffset, gap_upper), function: Function::Belt(Direction::Down) });
            pcb.add(Entity { location: Point::new(0, -total_instances_needed - 2) + Vector::new(input_xoffset, gap_upper), function: Function::Belt(Direction::Down) });
            
            available_outputs.insert(input, (0..total_instances_needed).map(|i| Point::new(i, -1) + Vector::new(input_xoffset, gap_upper)).collect());
            
            input_xoffset += total_instances_needed;
        }
        let global_output_point = Point::new(0, -1) + Vector::new(input_xoffset, gap_upper);
        pcb.add(Entity { location: global_output_point, function: Function::Belt(Direction::Up) });
        pcb.add(Entity { location: global_output_point + Vector::new(0, -1), function: Function::Belt(Direction::Up) });
        pcb.add(Entity { location: global_output_point + Vector::new(0, -2), function: Function::Belt(Direction::Up) });


        let col_vec = Vector::new(12, 0);
        let tile_vec = Vector::new(0, 4);
        for (col, &recipe) in order.iter().enumerate() {
            for incoming in graph.neighbors_directed(recipe, petgraph::Direction::Incoming) {
                println!("[{}] in {} cost={}", recipe, incoming, graph[(incoming, recipe)]);
            }

            let num_distinct_inputs = graph.neighbors_directed(recipe, petgraph::Direction::Incoming).count();

            let col_start = col_vec * (col as i32);
            let howmany_total: f64 = graph.neighbors_directed(recipe, petgraph::Direction::Outgoing).map(|x| graph[(recipe, x)]).sum();
            let howmany_total = howmany_total.ceil() as i32;
            println!("{} {}", recipe, howmany_total);
            for i in 0..howmany_total {
                let tile_start = col_start + tile_vec * i;
                if num_distinct_inputs > 2 {
                    pcb.add_all(&[
                        Entity { location: Point::new(0, 0) + tile_start, function: Function::Belt(Direction::Down) },
                        Entity { location: Point::new(0, 1) + tile_start, function: Function::Belt(Direction::Down) },
                        Entity { location: Point::new(0, 2) + tile_start, function: Function::Belt(Direction::Down) },
                        Entity { location: Point::new(0, 3) + tile_start, function: Function::Belt(Direction::Down) },
                        Entity { location: Point::new(2, 1) + tile_start, function: Function::Inserter { orientation: Direction::Right, long_handed: true } },
                    ]);
                }
                pcb.add_all(&[
                    Entity { location: Point::new(1, 0) + tile_start, function: Function::Belt(Direction::Down) },
                    Entity { location: Point::new(1, 1) + tile_start, function: Function::Belt(Direction::Down) },
                    Entity { location: Point::new(1, 2) + tile_start, function: Function::Belt(Direction::Down) },
                    Entity { location: Point::new(1, 3) + tile_start, function: Function::Belt(Direction::Down) },
                    Entity { location: Point::new(7, 0) + tile_start, function: Function::Belt(Direction::Up) },
                    Entity { location: Point::new(7, 1) + tile_start, function: Function::Belt(Direction::Up) },
                    Entity { location: Point::new(7, 2) + tile_start, function: Function::Belt(Direction::Up) },
                    Entity { location: Point::new(7, 3) + tile_start, function: Function::Belt(Direction::Up) },

                    Entity { location: Point::new(2, 2) + tile_start, function: Function::Inserter { orientation: Direction::Right, long_handed: false } },
                    Entity { location: Point::new(6, 1) + tile_start, function: Function::Inserter { orientation: Direction::Right, long_handed: false } },
                    Entity { location: Point::new(3, 0) + tile_start, function: function_map[recipe].clone() },
                    Entity { location: Point::new(2, 3) + tile_start, function: Function::ElectricPole },
                    Entity { location: Point::new(6, 3) + tile_start, function: Function::ElectricPole },
                ]);
            }

            let input_points = if num_distinct_inputs > 1 {
                pcb.replace(Entity { location: Point::new(0, 0) + col_start, function: Function::Belt(Direction::Right) });
                pcb.replace(Entity { location: Point::new(2, 0) + col_start, function: Function::Belt(Direction::Left) });
                let mut points = vec![Point::new(0, 0), Point::new(2, 0)];
                if num_distinct_inputs > 2 {
                    pcb.replace(Entity { location: Point::new(0, 3) + col_start + tile_vec * (howmany_total - 1), function: Function::Belt(Direction::Up) });
                    points.extend(if num_distinct_inputs > 3 {
                        pcb.replace(Entity { location: Point::new(-1, 0) + col_start, function: Function::Belt(Direction::Down) });
                        pcb.replace(Entity { location: Point::new(-1, 1) + col_start, function: Function::Belt(Direction::Right) });
                        pcb.replace(Entity { location: Point::new(-1, 2) + col_start, function: Function::Belt(Direction::Up) });
                        vec![Point::new(-1, 0), Point::new(-1, 2)]
                    } else {
                        vec![Point::new(0, 1)]
                    });
                }
                points
            } else {
                vec![Point::new(1, 0)]
            };

            for (input_name, input_point) in graph.neighbors_directed(recipe, petgraph::Direction::Incoming).zip(input_points) {
                if let Some(outlist) = available_outputs.get_mut(input_name) {
                    needed_wires.push((outlist.pop().unwrap(), input_point + col_start));
                }
            }

            pcb.replace(Entity { location: Point::new(1, 3) + col_start + tile_vec * (howmany_total - 1), function: Function::Belt(Direction::Up) });
            pcb.replace(Entity { location: Point::new(7, 0) + col_start, function: Function::Belt(Direction::Right) });
            pcb.add(Entity { location: Point::new(8, 0) + col_start, function: Function::Belt(Direction::Down) });
            let num_extra_output_paths = graph.neighbors_directed(recipe, petgraph::Direction::Outgoing).count() as i32 - 1;
            let mut output_nodes = Vec::new();
            for i in 0..num_extra_output_paths {
                let tile_start = col_start + Vector::new(8, i * 2 + 1);
                pcb.add_all(&[
                    Entity { location: Point::new(0, 0) + tile_start, function: Function::Splitter(Direction::Down) },
                    Entity { location: Point::new(0, 1) + tile_start, function: Function::Belt(Direction::Down) },
                    Entity { location: Point::new(1, 1) + tile_start, function: Function::Belt(Direction::Right) },
                ]);
                output_nodes.push(Point::new(1, 1) + tile_start);
            }
            pcb.add_all(&[
                Entity { location: Point::new(8, num_extra_output_paths * 2 + 1) + col_start, function: Function::Belt(Direction::Right) },
                Entity { location: Point::new(9, num_extra_output_paths * 2 + 1) + col_start, function: Function::Belt(Direction::Right) },
            ]);
            output_nodes.push(Point::new(9, num_extra_output_paths * 2 + 1) + col_start);

            output_nodes.reverse();
            available_outputs.insert(recipe, output_nodes);

            for outgoing in graph.neighbors_directed(recipe, petgraph::Direction::Outgoing) {
                println!("[{}] out {} cost={}", recipe, outgoing, graph[(recipe, outgoing)]);
            }
        }
        
        needed_wires.push((available_outputs.get_mut(&tree.output as &str).unwrap().pop().unwrap(), global_output_point));
        
        println!("{}", render::ascii(pcb));

        needed_wires
    }
}

