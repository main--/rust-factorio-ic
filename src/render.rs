use std::iter::{self, FromIterator};

use crate::pcb::{Pcb, Entity, Function, Direction};

#[must_use]
pub fn ascii(pcb: &Pcb) -> String {
    AsciiCanvas::build(pcb.entities()).render()
}

#[must_use]
pub fn ascii_wire_to_route(rows: &Vec<Vec<bool>>, from: (i32, i32), to: (i32, i32)) -> String {
    let mut res = String::with_capacity(1024);
    for (y, row) in rows.iter().enumerate() {
        for (x, val) in row.iter().copied().enumerate() {
            if (x as i32, y as i32) == to {
                res.push('T');
            } else if (x as i32, y as i32) == from {
                res.push('F');
            } else if val {
                res.push('X');
            } else {
                res.push(' ');
            }
        }
        res.push('\n');
    }
    res
}

#[must_use]
pub fn ascii_routed_wire(rows: &Vec<Vec<bool>>, path: &Vec<(i32, i32)>) -> String {
    let mut rows2 = rows
        .iter()
        .map(|x| x.iter().map(|&b| if b { 'X' } else { ' ' }).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    for (i, &(x, y)) in path.iter().enumerate() {
        let c = i.to_string().chars().last().unwrap();
        rows2[y as usize][x as usize] = c;
    }
    let mut res = String::with_capacity(1024);
    for row in &rows2 {
        for &x in row {
            res.push(x);
        }
        res.push('\n');
    }
    res
}

#[must_use]
pub fn blueprint(pcb: &Pcb) -> String {
    use factorio_blueprint::{objects::*, BlueprintCodec, Container};
    use std::convert::TryInto;

    let container = Container::Blueprint(Blueprint {
        item: "blueprint".to_owned(),
        label: "very cool".to_owned(),
        label_color: None,
        version: 77310525440,
        schedules: vec![],
        icons: vec![Icon {
            index: OneBasedIndex::new(1).unwrap(),
            signal: SignalID { name: "electronic-circuit".to_owned(), type_: SignalIDType::Item },
        }],
        tiles: vec![],
        entities: pcb.entities()
            .enumerate()
            .map(|(i, e)| {
                let mut underground_type = None;
                let mut recipe = None;
                let mut direction = None;
                let mut position = Position {
                    x: (e.location.x as f64).try_into().unwrap(),
                    y: (e.location.y as f64).try_into().unwrap(),
                };
                let name = match e.function {
                    Function::Assembler { recipe: ref r } => {
                        recipe = Some(r.clone());
                        position.x += 1.;
                        position.y += 1.;
                        "assembling-machine-2"
                    },
                    Function::Furnace => {
                        position.x += 1.;
                        position.y += 1.;
                        "electric-furnace"
                    }
                    Function::Inserter { orientation, long_handed } => {
                        // reverse direction because the game thinks about these differently than we
                        // do
                        direction = Some(match orientation {
                            Direction::Up => Direction::Down,
                            Direction::Down => Direction::Up,
                            Direction::Left => Direction::Right,
                            Direction::Right => Direction::Left,
                        });
                        if long_handed { "long-handed-inserter" } else { "inserter" }
                    },
                    Function::Belt(d) => {
                        direction = Some(d);
                        "transport-belt"
                    },
                    Function::UndergroundBelt(d, down) => {
                        direction = Some(d);
                        underground_type =
                            Some(if down { EntityType::Input } else { EntityType::Output });
                        "underground-belt"
                    },
                };

                Entity {
                    entity_number: EntityNumber::new(i + 1).unwrap(),
                    name: name.to_owned(),
                    position,
                    direction: direction.map(|d| match d {
                        Direction::Up => 0,
                        Direction::Right => 2,
                        Direction::Down => 4,
                        Direction::Left => 6,
                    }),
                    orientation: None,
                    connections: None,
                    control_behaviour: None,
                    items: None,
                    recipe,
                    bar: None,
                    inventory: None,
                    infinity_settings: None,
                    type_: underground_type,
                    input_priority: None,
                    output_priority: None,
                    filter: None,
                    filters: None,
                    filter_mode: None,
                    override_stack_size: None,
                    drop_position: None,
                    pickup_position: None,
                    request_filters: None,
                    request_from_buffers: None,
                    parameters: None,
                    alert_parameters: None,
                    auto_launch: None,
                    variation: None,
                    color: None,
                    station: None,
                }
            })
            .collect(),
    });
    BlueprintCodec::encode_string(&container).unwrap()
}

struct AsciiCanvas {
    offset_x: i32,
    offset_y: i32,
    canvas: Vec<Vec<char>>,
}
impl AsciiCanvas {
    fn build<'a>(entities: impl Clone + Iterator<Item=&'a Entity>) -> Self {
        let min_x = entities.clone().map(|x| x.location.x).min().unwrap_or(0);
        let min_y = entities.clone().map(|x| x.location.y).min().unwrap_or(0);
        let max_x = entities.clone().map(|x| x.location.x + x.size_x()).max().unwrap_or(0);
        let max_y = entities.clone().map(|x| x.location.y + x.size_y()).max().unwrap_or(0);

        let offset_x = -min_x;
        let offset_y = -min_y;
        let size_x = max_x + offset_x;
        let size_y = max_y + offset_y;

        let canvas_row: Vec<char> = iter::repeat(' ').take(size_x as usize).collect();
        let mut canvas = AsciiCanvas {
            canvas: iter::repeat(canvas_row).take(size_y as usize).collect(),
            offset_x,
            offset_y,
        };

        for e in entities {
            match e.function {
                Function::Assembler { ref recipe } => {
                    canvas.set(e.location.x + 0, e.location.y + 0, '┌');
                    canvas.set(e.location.x + 1, e.location.y + 0, '─');
                    canvas.set(e.location.x + 2, e.location.y + 0, '┐');
                    canvas.set(e.location.x + 0, e.location.y + 1, '│');
                    canvas.set(e.location.x + 1, e.location.y + 1, recipe.to_uppercase().chars().next().unwrap());
                    canvas.set(e.location.x + 2, e.location.y + 1, '│');
                    canvas.set(e.location.x + 0, e.location.y + 2, '└');
                    canvas.set(e.location.x + 1, e.location.y + 2, '─');
                    canvas.set(e.location.x + 2, e.location.y + 2, '┘');
                },
                Function::Furnace => {
                    canvas.set(e.location.x + 0, e.location.y + 0, '┌');
                    canvas.set(e.location.x + 1, e.location.y + 0, '─');
                    canvas.set(e.location.x + 2, e.location.y + 0, '┐');
                    canvas.set(e.location.x + 0, e.location.y + 1, '│');

                    canvas.set(e.location.x + 2, e.location.y + 1, '│');
                    canvas.set(e.location.x + 0, e.location.y + 2, '└');
                    canvas.set(e.location.x + 1, e.location.y + 2, '─');
                    canvas.set(e.location.x + 2, e.location.y + 2, '┘');
                }
                Function::Inserter { orientation: d, long_handed } => {
                    let symbol = if long_handed {
                        match d {
                            Direction::Up => '↟',
                            Direction::Down => '↡',
                            Direction::Left => '↞',
                            Direction::Right => '↠',
                        }
                    } else {
                        match d {
                            Direction::Up => '↑',
                            Direction::Down => '↓',
                            Direction::Left => '←',
                            Direction::Right => '→',
                        }
                    };
                    canvas.set(e.location.x, e.location.y, symbol);
                },
                Function::Belt(d) => {
                    let symbol = match d {
                        Direction::Up => '⍐',
                        Direction::Down => '⍗',
                        Direction::Left => '⍇',
                        Direction::Right => '⍈',
                    };
                    canvas.set(e.location.x, e.location.y, symbol);
                },
                Function::UndergroundBelt(d, down) => {
                    let symbol = if down {
                        match d {
                            Direction::Up => '⍓',
                            Direction::Down => '⍌',
                            Direction::Left => '⍃',
                            Direction::Right => '⍄',
                        }
                    } else {
                        match d {
                            Direction::Up => '⍌',
                            Direction::Down => '⍓',
                            Direction::Left => '⍄',
                            Direction::Right => '⍃',
                        }
                    };
                    canvas.set(e.location.x, e.location.y, symbol);
                },
            }
        }

        canvas
    }

    fn set(&mut self, x: i32, y: i32, c: char) {
        self.canvas[(y + self.offset_y) as usize][(x + self.offset_x) as usize] = c;
    }

    fn render(&self) -> String {
        self.canvas.iter().map(String::from_iter).collect::<Vec<_>>().join("\n")
    }
}

