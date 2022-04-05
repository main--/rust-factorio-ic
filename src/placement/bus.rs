//! bus-style placer

use std::iter;

use crate::{Entity, Direction, Function, Rational};
use crate::kirkmcdonald::ProductionGraph;
use crate::pcb::{Pcb, Point, Vector, NeededWires, need_belt, WireKind, NeededWire};
use crate::recipe::Category;
use crate::render;
use super::Placer;

use fnv::FnvHashMap;
use petgraph::prelude::*;

pub struct BusPlacer;

static OUTPUT: &'static str = "<output>";

#[derive(Debug, Clone, Copy)]
struct Edge {
    num_assemblers: Rational,
    items_per_second: Rational,
}

impl Placer for BusPlacer {
    fn place(pcb: &mut impl Pcb, tree: &ProductionGraph) -> NeededWires {
        let mut needed_wires = NeededWires::new();

        // 1. calculate how much we need (i.e. flatten the production graph)
        let mut graph = DiGraphMap::<&str, Edge>::new();
        let mut function_map = FnvHashMap::default();
        let mut kind_map = FnvHashMap::default();

        let mut todo_stack = vec![tree];
        while let Some(item) = todo_stack.pop() {
            kind_map.insert(item.output.as_str(), item.output_kind.clone());

            if item.building != Some(Category::Assembler) && item.building != Some(Category::Furnace) {
                continue;
            }

            todo_stack.extend(&item.inputs);

            for input in &item.inputs {
                match graph.edge_weight_mut(&input.output, &item.output) {
                    Some(existing) => {
                        existing.num_assemblers += input.how_many;
                        existing.items_per_second += input.per_second;
                    }
                    None => {
                        graph.add_edge(&input.output, &item.output, Edge {
                            num_assemblers: input.how_many,
                            items_per_second: input.per_second
                        });
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

        graph.add_edge(&tree.output, OUTPUT, Edge { num_assemblers: tree.how_many, items_per_second: tree.per_second });

        // 2. build global inputs for the stuff we can't produce (i.e. ores, fluids, right now also chemical plant products like plastic and batteries)
        let mut global_inputs = Vec::new();
        for i in (0..order.len()).rev() {
            if graph.neighbors_directed(order[i], petgraph::Direction::Incoming).count() == 0 {
                global_inputs.push(order.remove(i));
            }
        }

        let mut available_outputs = FnvHashMap::<&str, Vec<Point>>::default();

        let lane_throughput = Rational::new(15, 2);

        let gap_upper = -10;
        let mut input_xoffset = 5;
        for input in global_inputs {
            let kind = kind_map.get(input).unwrap();

            let total_instances_needed: i32 = graph.neighbors_directed(input, petgraph::Direction::Outgoing).map(|e| (graph[(input, e)].items_per_second / lane_throughput).ceil().to_integer()).sum();
            for i in 1..total_instances_needed { // FIXME: this loop is untested, not sure how to trigger it
                for j in 0..(total_instances_needed-2) {
                    pcb.add(Entity { location: Point::new(j, -i) + Vector::new(input_xoffset, gap_upper), function: Function::Belt(Direction::Down) });
                }
                pcb.add(Entity { location: Point::new(total_instances_needed-2, -i-1) + Vector::new(input_xoffset, gap_upper), function: Function::Splitter(Direction::Down) });
                pcb.add(Entity { location: Point::new(total_instances_needed-2, -i) + Vector::new(input_xoffset, gap_upper), function: Function::Belt(Direction::Down) });
                pcb.add(Entity { location: Point::new(total_instances_needed-1, -i) + Vector::new(input_xoffset, gap_upper), function: Function::Belt(Direction::Down) });
            }
            if total_instances_needed == 1 {
                pcb.add(Entity { location: Point::new(0, -total_instances_needed) + Vector::new(input_xoffset, gap_upper), function: Function::Belt(Direction::Down) });
            }

            let input_name = match kind {
                WireKind::Belt => input.to_owned(),
                WireKind::Pipe(_) => format!("{}-barrel", input),
            };
            pcb.add(Entity { location: Point::new(0, -total_instances_needed - 1) + Vector::new(input_xoffset, gap_upper), function: Function::InputMarker(input_name) });
            pcb.add(Entity { location: Point::new(0, -total_instances_needed - 2) + Vector::new(input_xoffset, gap_upper), function: Function::Belt(Direction::Down) });

            available_outputs.insert(input, (0..total_instances_needed).map(|i| Point::new(i, -1) + Vector::new(input_xoffset, gap_upper)).collect());

            input_xoffset += total_instances_needed + 2;
        }

        // 3. global output
        let global_output_point = Point::new(0, -1) + Vector::new(input_xoffset, gap_upper);
        pcb.add(Entity { location: global_output_point, function: Function::Belt(Direction::Up) });
        pcb.add(Entity { location: global_output_point + Vector::new(0, -1), function: Function::Belt(Direction::Up) });
        pcb.add(Entity { location: global_output_point + Vector::new(0, -2), function: Function::Belt(Direction::Up) });


        // 4. bus nodes!
        // Vocabulary:
        // "column" / "unit": bus node
        // "subunit": bus node that was created when splitting a bus node into multiple smaller ones
        // "tile": one assembler + supporting structures (inserter, belt, power etc) inside a bus node
        struct BusNode<'a> {
            max_assemblers_per_unit: i32,
            num_assemblers_total: Rational,
            items_out_per_second_per_assembler: Rational,
            items_in_per_second_per_assembler: FnvHashMap<&'a str, Rational>,
            pipe_input: Option<&'a str>,
        }
        impl<'a> BusNode<'a> {
            /// One element per unit describing the number of assemblers in that unit
            fn units(&self) -> impl Iterator<Item=Rational> {
                let mut assemblers = self.num_assemblers_total;
                let mapu = Rational::from(self.max_assemblers_per_unit);
                iter::from_fn(move || {
                    let i = std::cmp::min(assemblers, mapu);
                    assemblers -= i;
                    Some(i)
                }).take_while(|&i| i != Rational::from(0))
            }
            /// Desired input belts by item type and throughput
            fn desired_input_belts(&self) -> impl Iterator<Item=(&'a str, Rational)> + '_ {
                self.units().flat_map(|i| self.items_in_per_second_per_assembler.iter().map(move |(&k, &v)| (k, v * i)))
            }
            /// Number of required belt input lanes
            fn num_distinct_inputs(&self) -> usize {
                self.items_in_per_second_per_assembler.len()
            }
            /// Belt input recipes
            fn belt_inputs(&self) -> impl Iterator<Item=&'a str> + '_ {
                self.items_in_per_second_per_assembler.keys().copied()
            }
        }
        let mut bus_nodes = FnvHashMap::default();
        for &recipe in order.iter() {
            let input_edges = graph.neighbors_directed(recipe, petgraph::Direction::Incoming);
            let output_edges = graph.neighbors_directed(recipe, petgraph::Direction::Outgoing);
            for incoming in input_edges.clone() {
                println!("[{}] in {} cost={}", recipe, incoming, graph[(incoming, recipe)].num_assemblers);
            }

            let belt_inputs = input_edges.clone().filter(|c| *kind_map.get(c).unwrap() == WireKind::Belt);
            let pipe_input = input_edges.clone().filter(|c| *kind_map.get(c).unwrap() != WireKind::Belt).next();

            let howmany_exact: Rational = output_edges.clone().map(|x| graph[(recipe, x)].num_assemblers).sum();

            let items_in_per_second_per_assembler = belt_inputs.clone().map(|i| (i, graph[(i, recipe)])).map(|(i, e)| (i, e.items_per_second / howmany_exact)).collect();
            let in_max_throughput = belt_inputs.clone().map(|i| graph[(i, recipe)]).map(|e| e.items_per_second / howmany_exact).max().unwrap();
            let out_throughput = output_edges.clone().map(|o| graph[(recipe, o)]).map(|e| e.items_per_second / e.num_assemblers).next().unwrap();
            let io_max_throughput = std::cmp::max(in_max_throughput, out_throughput);

            let max_assemblers_per_unit = (lane_throughput / io_max_throughput).floor();
            println!("[{}] MAPU = {}", recipe, max_assemblers_per_unit);
            if max_assemblers_per_unit < Rational::from(1) {
                panic!("One assembler of {} produces more output than one lane can handle", recipe);
            }

            bus_nodes.insert(recipe, BusNode {
                max_assemblers_per_unit: max_assemblers_per_unit.to_integer(),
                num_assemblers_total: howmany_exact,
                items_out_per_second_per_assembler: out_throughput,
                items_in_per_second_per_assembler,
                pipe_input,
            });
        }
        bus_nodes.insert(OUTPUT, BusNode {
            max_assemblers_per_unit: 1,
            num_assemblers_total: Rational::from(1),
            items_out_per_second_per_assembler: Rational::from(0),
            items_in_per_second_per_assembler: FnvHashMap::from_iter(std::iter::once((tree.output.as_str(), Rational::from(0)))),
            pipe_input: None
        });

        let col_vec = Vector::new(12, 0);
        let tile_vec = Vector::new(0, 4);
        let mut cols_counter = 0;
        for &recipe in order.iter() {
            let output_edges = graph.neighbors_directed(recipe, petgraph::Direction::Outgoing);

            let node = bus_nodes.get(recipe).unwrap();

            #[derive(Clone, Copy, Debug)]
            struct OutputBeltCarry {
                end: Point,
                flow: Rational,
            }
            let mut output_belt_carry: Option<OutputBeltCarry> = None;

            let ox = node.pipe_input.is_some() as i32;

            //let num_extra_output_paths = graph.neighbors_directed(recipe, petgraph::Direction::Outgoing).count() as i32 - 1;
            let mut consumers: Vec<_> = output_edges.clone().flat_map(|e| bus_nodes.get(e).unwrap().desired_input_belts().filter(|&(k, _)| k == recipe).map(|(_, v)| v)).collect();
            consumers.sort(); // sort biggest consumers to the back (where we start popping)
            //println!("consumers for ");
            // here we employ the following algorithm:
            // - sort biggest consumers first
            // - for each subunit, look at the start of this list
            // - prepare output pads as long as we can satisfy consumers
            // - once we can't, make a carryover connection to the next subunit's first splitter
            // - over there we continue the same algorithm
            // - if we somehow don't satisfy all consumers, we have a bug somewhere
            // Note: If the subunit (due to input bottlenecks) is not able to satisfy the biggest consumer,
            // this means that we will directly carry it over to the second one and try again there.
            // Note 2: If the second subunit is still not enough, keep going. This is noteworthy because we
            // have to make sure to build an output splitter for it even though we have only one output path in this case.
            // Note 3: This feels dangerous because combining a flow of e.g. 2x 0.9 lanes on a splitter will
            // create a bottleneck unless the first consumer immediately takes at least 0.8 lanes of output.
            // However, there is a very simple correctness proof: If we can't wire up a consumer because it
            // consumes more than X, and the next unit produces at most one full lane (by definition), then,
            // because the consumer eats MORE than X, we are always left with less than a full lane. qed

            // split this into multiple units if needed due to belt throughput bottlenecks
            for howmany_total in node.units() {
                let howmany_total = howmany_total.ceil().to_integer();
                println!("[{}] {} assemblers", recipe, howmany_total);

                let col_start = col_vec * (cols_counter as i32);

                println!("{} {}", recipe, howmany_total);
                for i in 0..howmany_total {
                    let tile_start = col_start + tile_vec * i;
                    if node.num_distinct_inputs() > 2 {
                        // extra input belt and long inserter
                        pcb.add_all(&[
                            Entity { location: Point::new(0, 0) + tile_start, function: Function::Belt(Direction::Down) },
                            Entity { location: Point::new(0, 1) + tile_start, function: Function::Belt(Direction::Down) },
                            Entity { location: Point::new(0, 2) + tile_start, function: Function::Belt(Direction::Down) },
                            Entity { location: Point::new(0, 3) + tile_start, function: Function::Belt(Direction::Down) },
                            Entity { location: Point::new(2, 1) + tile_start, function: Function::Inserter { orientation: Direction::Right, long_handed: true } },
                        ]);
                    }
                    // primary components: assembler, electricity, belts, inserters
                    pcb.add_all(&[
                        Entity { location: Point::new(1, 0) + tile_start, function: Function::Belt(Direction::Down) },
                        Entity { location: Point::new(1, 1) + tile_start, function: Function::Belt(Direction::Down) },
                        Entity { location: Point::new(1, 2) + tile_start, function: Function::Belt(Direction::Down) },
                        Entity { location: Point::new(1, 3) + tile_start, function: Function::Belt(Direction::Down) },
                        Entity { location: Point::new(7 + ox, 0) + tile_start, function: Function::Belt(Direction::Up) },
                        Entity { location: Point::new(7 + ox, 1) + tile_start, function: Function::Belt(Direction::Up) },
                        Entity { location: Point::new(7 + ox, 2) + tile_start, function: Function::Belt(Direction::Up) },
                        Entity { location: Point::new(7 + ox, 3) + tile_start, function: Function::Belt(Direction::Up) },

                        Entity { location: Point::new(2, 2) + tile_start, function: Function::Inserter { orientation: Direction::Right, long_handed: false } },
                        Entity { location: Point::new(6, 2) + tile_start, function: Function::Inserter { orientation: Direction::Right, long_handed: node.pipe_input.is_some() } },
                        Entity { location: Point::new(3, 0) + tile_start, function: function_map[recipe].clone() },
                        Entity { location: Point::new(2, 3) + tile_start, function: Function::ElectricPole },
                        Entity { location: Point::new(6, 3) + tile_start, function: Function::ElectricPole },
                    ]);

                    // fluid input to the right
                    if let Some(pipe_in) = node.pipe_input {
                        pcb.add_all(&[
                            Entity { location: Point::new(7, 0) + tile_start, function: Function::Pipe(pipe_in.to_owned()) },
                            Entity { location: Point::new(7, 1) + tile_start, function: Function::Pipe(pipe_in.to_owned()) },
                            Entity { location: Point::new(6, 1) + tile_start, function: Function::Pipe(pipe_in.to_owned()) },
                            Entity { location: Point::new(7, 2) + tile_start, function: Function::Pipe(pipe_in.to_owned()) },
                            Entity { location: Point::new(7, 3) + tile_start, function: Function::Pipe(pipe_in.to_owned()) },
                        ]);
                    }
                }

                let input_points = if node.num_distinct_inputs() > 1 {
                    // combine two input lanes on primary input belt
                    pcb.replace(Entity { location: Point::new(0, 0) + col_start, function: Function::Belt(Direction::Right) });
                    pcb.replace(Entity { location: Point::new(2, 0) + col_start, function: Function::Belt(Direction::Left) });
                    let mut points = vec![Point::new(0, 0), Point::new(2, 0)];
                    if node.num_distinct_inputs() > 2 {
                        pcb.replace(Entity { location: Point::new(0, 3) + col_start + tile_vec * (howmany_total - 1), function: Function::Belt(Direction::Up) });
                        if node.num_distinct_inputs() > 3 {
                            // combine two input lanes on secondary input belt
                            pcb.replace(Entity { location: Point::new(-1, 0) + col_start, function: Function::Belt(Direction::Down) });
                            pcb.replace(Entity { location: Point::new(-1, 1) + col_start, function: Function::Belt(Direction::Right) });
                            pcb.replace(Entity { location: Point::new(-1, 2) + col_start, function: Function::Belt(Direction::Up) });
                            points.extend(&[Point::new(-1, 0), Point::new(-1, 2)]);
                        } else {
                            // secondary input belt is a single lane
                            points.push(Point::new(0, 1));
                        };
                    }
                    points
                } else {
                    // primary input belt is a single lane
                    vec![Point::new(1, 0)]
                };


                // request wire connections towards our belt inputs
                for (input_name, input_point) in node.belt_inputs().zip(input_points) {
                    if let Some(from) = available_outputs.get_mut(input_name).and_then(|outlist| outlist.pop()) {
                        needed_wires.push(NeededWire {
                            from,
                            to: input_point + col_start,
                            wire_kind: kind_map.get(input_name).unwrap().clone(),
                        });
                    }
                }
                // fluid inputs as well
                if let Some(pipe_input) = node.pipe_input {
                    if let Some(outlist) = available_outputs.get_mut(pipe_input) {
                        needed_wires.push(NeededWire {
                            from: outlist.pop().unwrap(),
                            to: Point::new(7, 0) + col_start,
                            wire_kind: WireKind::Pipe(pipe_input.to_owned()),
                        });
                    }
                }

                // safely terminate primary input belt
                pcb.replace(Entity { location: Point::new(1, 3) + col_start + tile_vec * (howmany_total - 1), function: Function::Belt(Direction::Up) });

                let mut flow = node.items_out_per_second_per_assembler * howmany_total;
                if let Some(carry) = output_belt_carry.as_ref() {
                    let to = Point::new(9 + ox, 0) + col_start;
                    needed_wires.push(NeededWire {
                        from: carry.end,
                        to,
                        wire_kind: WireKind::Belt
                    });
                    pcb.add(Entity { location: to, function: Function::Belt(Direction::Down) });
                    flow += carry.flow;
                }

                let mut num_output_paths = 0;
                while let Some(&consumer) = consumers.last() {
                    if consumer <= flow {
                        consumers.pop();
                        flow -= consumer;
                        num_output_paths += 1;
                    } else {
                        break;
                    }
                }

                // needs a carry
                if !consumers.is_empty() {
                    num_output_paths += 1;
                }


                // split up outputs
                pcb.replace(Entity { location: Point::new(7 + ox, 0) + col_start, function: Function::Belt(Direction::Right) });
                pcb.add(Entity { location: Point::new(8 + ox, 0) + col_start, function: Function::Belt(Direction::Down) });
                let mut output_nodes = Vec::new();
                for i in 1..num_output_paths {
                    let tile_start = col_start + Vector::new(8 + ox, i * 2 - 1);
                    pcb.add_all(&[
                        Entity { location: Point::new(0, 0) + tile_start, function: Function::Splitter(Direction::Down) },
                        Entity { location: Point::new(0, 1) + tile_start, function: Function::Belt(Direction::Down) },
                        Entity { location: Point::new(1, 1) + tile_start, function: Function::Belt(Direction::Right) },
                    ]);
                    output_nodes.push(Point::new(1, 1) + tile_start);
                }
                pcb.add_all(&[
                    Entity { location: Point::new(8 + ox, num_output_paths * 2 - 1) + col_start, function: Function::Belt(Direction::Right) },
                    Entity { location: Point::new(9 + ox, num_output_paths * 2 - 1) + col_start, function: Function::Belt(Direction::Right) },
                ]);
                let default_out_point = Point::new(9 + ox, num_output_paths * 2 - 1) + col_start;
                if (flow > Rational::from(0)) && !consumers.is_empty() {
                    output_belt_carry = Some(OutputBeltCarry {
                        end: default_out_point,
                        flow,
                    });
                } else {
                    output_nodes.push(default_out_point);
                    output_belt_carry = None;
                }


                output_nodes.reverse();
                available_outputs.entry(recipe).or_default().extend_from_slice(&output_nodes);

                for outgoing in output_edges.clone() {
                    println!("[{}] out {} cost={}", recipe, outgoing, graph[(recipe, outgoing)].num_assemblers);
                }
                cols_counter += 1;
            }
            assert!(consumers.is_empty());
        }

        // 5. wire up the output to the last bus node
        // (can't do this earlier because the output belt's exact position is only known here)
        needed_wires.push(need_belt(available_outputs.get_mut(&tree.output as &str).unwrap().pop().unwrap(), global_output_point));

        println!("{}", render::ascii(pcb));

        needed_wires
    }
}

