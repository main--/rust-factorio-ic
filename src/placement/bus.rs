//! bus-style placer

use std::cell::RefCell;
use std::iter;

use crate::consts::Constants;
use crate::{Entity, Direction, Function, Rational};
use crate::kirkmcdonald::ProductionGraph;
use crate::pcb::{Pcb, Point, Vector, NeededWires, need_belt, WireKind, NeededWire, InserterKind};
use crate::recipe::Category;
use crate::render;
use super::Placer;

use fnv::FnvHashMap;
use itertools::Itertools;
use petgraph::prelude::*;

pub struct BusPlacer;

static OUTPUT: &'static str = "<output>";

#[derive(Debug, Clone, Copy)]
struct Edge {
    num_assemblers: Rational,
    items_per_second: Rational,
}

impl Placer for BusPlacer {
    fn place(pcb: &mut impl Pcb, tree: &ProductionGraph, consts: &Constants) -> NeededWires {
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

        let lane_throughput = consts.max_belts.lane_items_per_second();

        let gap_upper = -10;
        let mut input_xoffset = 5;
        for input in global_inputs {
            let kind = kind_map.get(input).unwrap();

            let total_instances_needed: i32 = graph.neighbors_directed(input, petgraph::Direction::Outgoing).map(|e| (graph[(input, e)].items_per_second / lane_throughput).ceil().to_integer()).sum();
            for i in 1..total_instances_needed {
                for j in 0..(total_instances_needed-i-1) {
                    pcb.add(Entity { location: Point::new(j, -i-1) + Vector::new(input_xoffset, gap_upper), function: Function::Belt(Direction::Down) });
                }
                pcb.add(Entity { location: Point::new(total_instances_needed-1-i, -i-1) + Vector::new(input_xoffset, gap_upper), function: Function::Splitter(Direction::Down) });
            }

            for i in 0..total_instances_needed {
                pcb.add(Entity { location: Point::new(i, -1) + Vector::new(input_xoffset, gap_upper), function: Function::Belt(Direction::Down) });
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
            belt_inputs: Vec<BusNodeInput<'a>>,
            pipe_input: Option<&'a str>,
            primary_inserter_kind: InserterKind,
            out_serter_kind: InserterKind,

            belt_inbox: RefCell<FnvHashMap<&'a str, Vec<Point>>>,
        }
        struct BusNodeInput<'a> {
            name: &'a str,
            items_per_second_per_assembler: Rational,
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
                self.units().flat_map(|i| self.belt_inputs.iter().map(move |x| (x.name, x.items_per_second_per_assembler * i)))
            }
            /// Number of required belt input lanes
            fn num_distinct_inputs(&self) -> usize {
                self.belt_inputs.len()
            }
            /// Belt input recipes
            fn belt_inputs(&self) -> impl Iterator<Item=&'a str> + '_ {
                self.belt_inputs.iter().map(|x| x.name)
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

            let find_inserter_kind = |bw: Rational, force_long: bool, name: &str| -> InserterKind {
                if force_long {
                    if bw > consts.long_inserter_items_per_second() {
                        panic!("{} throughput of {} is too much for the long inserter!", name, bw);
                    }
                    return InserterKind::LongHanded;
                }

                if bw <= consts.basic_inserter_items_per_second() {
                    InserterKind::Normal
                } else if bw <= consts.fast_inserter_items_per_second() {
                    InserterKind::Fast
                } else if bw <= consts.stack_inserter_items_per_second() {
                    InserterKind::Stack
                } else {
                    panic!("{} throughput of {} is too much for even a stack inserter!", name, bw);
                }
            };

            let mut inputs: Vec<_> = belt_inputs.clone().map(|i| (i, graph[(i, recipe)])).map(|(i, e)| BusNodeInput { name: i, items_per_second_per_assembler: e.items_per_second / howmany_exact }).collect();
            let long_inserter_tp = consts.long_inserter_items_per_second();
            let (_, secondary_belt_inputs) = (0..inputs.len())
                .combinations(inputs.len().saturating_sub(2))
                .filter(|c| c.iter().copied().all_unique())
                .map(|c| (c.iter().map(|&i| inputs[i].items_per_second_per_assembler).sum::<Rational>(), c))
                .filter(|&(t, _)| t <= long_inserter_tp)
                .max_by_key(|&(t, _)| t)
                .expect("Secondary belt input bandwidth too high; long-handed inserter can't keep up!");
            for (i, input_idx) in secondary_belt_inputs.into_iter().enumerate() {
                inputs.swap(i + 2, input_idx);
            }
            let primary_inp_bw: Rational = inputs.iter().take(2).map(|c| c.items_per_second_per_assembler).sum();
            let primary_inserter_kind = find_inserter_kind(primary_inp_bw, false, "Primary input belt");

            let in_max_throughput = belt_inputs.clone().map(|i| graph[(i, recipe)]).map(|e| e.items_per_second / howmany_exact).max().unwrap();
            let out_throughput = output_edges.clone().map(|o| graph[(recipe, o)]).map(|e| e.items_per_second / e.num_assemblers).next().unwrap();
            let io_max_throughput = std::cmp::max(in_max_throughput, out_throughput);

            let out_serter_kind = find_inserter_kind(out_throughput, pipe_input.is_some(), "Output");

            let max_assemblers_per_unit = (lane_throughput / io_max_throughput).floor();
            println!("[{}] MAPU = {}", recipe, max_assemblers_per_unit);
            if max_assemblers_per_unit < Rational::from(1) {
                panic!("One assembler of {} produces more output than one lane can handle", recipe);
            }

            bus_nodes.insert(recipe, BusNode {
                max_assemblers_per_unit: max_assemblers_per_unit.to_integer(),
                num_assemblers_total: howmany_exact,
                items_out_per_second_per_assembler: out_throughput,
                belt_inputs: inputs,
                pipe_input,
                primary_inserter_kind,
                out_serter_kind,
                belt_inbox: RefCell::default(),
            });
        }
        bus_nodes.insert(OUTPUT, BusNode {
            max_assemblers_per_unit: 1,
            num_assemblers_total: Rational::from(1),
            items_out_per_second_per_assembler: Rational::from(0),
            belt_inputs: vec![BusNodeInput { name: tree.output.as_str(), items_per_second_per_assembler: Rational::from(0) }],
            pipe_input: None,
            primary_inserter_kind: InserterKind::Normal,
            out_serter_kind: InserterKind::Normal,
            belt_inbox: RefCell::default(),
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

            let mut consumers: Vec<_> = output_edges.clone()
                .map(|e| bus_nodes.get(e).unwrap())
                .flat_map(|n| n.desired_input_belts().filter(|&(k, _)| k == recipe).map(move |(_, v)| (v, n))).collect();
            consumers.sort_by_key(|x| x.0); // sort biggest consumers to the back (where we start popping)
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
                            Entity { location: Point::new(2, 1) + tile_start, function: Function::Inserter { orientation: Direction::Right, kind: InserterKind::LongHanded } },
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

                        Entity { location: Point::new(2, 2) + tile_start, function: Function::Inserter { orientation: Direction::Right, kind: node.primary_inserter_kind } },
                        Entity { location: Point::new(6, 2) + tile_start, function: Function::Inserter { orientation: Direction::Right, kind: node.out_serter_kind } },
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
                    let direct_feed = node.belt_inbox.borrow_mut().get_mut(input_name).and_then(|ol| ol.pop());
                    let from = direct_feed.or_else(|| available_outputs.get_mut(input_name).and_then(|outlist| outlist.pop()));
                    if let Some(from) = from {
                        needed_wires.push(NeededWire {
                            from,
                            to: input_point + col_start,
                            wire_kind: kind_map.get(input_name).unwrap().clone(),
                        });
                    }
                }
                // fluid inputs as well
                if let Some(pipe_input) = node.pipe_input {
                    // daisy-chain fluids
                    let to = Point::new(7, 0) + col_start;
                    let from = available_outputs.get_mut(pipe_input).and_then(|outlist| {
                        match outlist.pop() {
                            None => None,
                            Some(x) => {
                                outlist.push(to);
                                Some(x)
                            }
                        }
                    });
                    if let Some(from) = from {
                        needed_wires.push(NeededWire {
                            from,
                            to,
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

                let mut consumers_here = Vec::new();
                let mut num_output_paths = 0;
                while let Some(&(consumer_flow, consumer)) = consumers.last() {
                    if consumer_flow <= flow {
                        consumers.pop();
                        flow -= consumer_flow;
                        num_output_paths += 1;
                        consumers_here.push(consumer);
                    } else {
                        break;
                    }
                }

                // needs a carry
                let needs_carry = (flow > Rational::from(0)) && !consumers.is_empty();
                if needs_carry {
                    num_output_paths += 1;
                }

                let synth_splitter_for_carry_in = output_belt_carry.is_some() && num_output_paths == 1;
                if synth_splitter_for_carry_in {
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

                if synth_splitter_for_carry_in {
                    pcb.replace(Entity { location: Point::new(8+ox, 2) + col_start, function: Function::Belt(Direction::Up) });
                } else {
                    pcb.add_all(&[
                        Entity { location: Point::new(8 + ox, num_output_paths * 2 - 1) + col_start, function: Function::Belt(Direction::Right) },
                        Entity { location: Point::new(9 + ox, num_output_paths * 2 - 1) + col_start, function: Function::Belt(Direction::Right) },
                    ]);
                    let default_out_point = Point::new(9 + ox, num_output_paths * 2 - 1) + col_start;
                    output_nodes.push(default_out_point);
                }

                if needs_carry {
                    output_belt_carry = Some(OutputBeltCarry {
                        end: output_nodes.pop().unwrap(),
                        flow,
                    });
                } else {
                    output_belt_carry = None;
                }


                output_nodes.reverse();
                //available_outputs.entry(recipe).or_default().extend_from_slice(&output_nodes);
                assert_eq!(output_nodes.len(), consumers_here.len());
                for (point, customer) in output_nodes.into_iter().zip(consumers_here) {
                    customer.belt_inbox.borrow_mut().entry(recipe).or_default().insert(0, point);
                }

                for outgoing in output_edges.clone() {
                    println!("[{}] out {} cost={}", recipe, outgoing, graph[(recipe, outgoing)].num_assemblers);
                }
                cols_counter += 1;
            }
            assert!(consumers.is_empty());
        }

        // 5. wire up the output to the last bus node
        // (can't do this earlier because the output belt's exact position is only known here)
        //let final_output_belt = available_outputs.get_mut(&tree.output as &str).unwrap().pop().unwrap();
        let final_output_belt = *bus_nodes.get(OUTPUT).unwrap().belt_inbox.borrow().get(tree.output.as_str()).unwrap().last().unwrap();
        needed_wires.push(need_belt(final_output_belt, global_output_point));

        println!("{}", render::ascii(pcb));

        needed_wires
    }
}

