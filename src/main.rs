use std::iter::{self, FromIterator};
use rlua::{Table, Lua, Result};

#[derive(Clone, Copy)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

enum Function {
    Assembler { recipe: String },
    Inserter { orientation: Direction, long_handed: bool },
    Belt(Direction),
    UndergroundBelt(Direction, bool),
}
struct Entity {
    x: i32,
    y: i32,
    function: Function,
}
impl Entity {
    fn size_x(&self) -> i32 {
        match self.function {
            Function::Belt(_) | Function::UndergroundBelt(_, _) | Function::Inserter { .. } => 1,
            Function::Assembler { .. } => 3,
        }
    }
    fn size_y(&self) -> i32 {
        self.size_x() // currently everything is quadratic
    }
    fn overlaps(&self, x: i32, y: i32) -> bool {
        (self.x <= x) && (self.x + self.size_x() > x) &&
            (self.y <= y) && (self.y + self.size_y() > y)
    }
}

struct AsciiCanvas {
    offset_x: i32,
    offset_y: i32,
    canvas: Vec<Vec<char>>,
}
impl AsciiCanvas {
    fn build(entities: &[Entity]) -> Self {
        let min_x = entities.iter().map(|x| x.x).min().unwrap_or(0);
        let min_y = entities.iter().map(|x| x.y).min().unwrap_or(0);
        let max_x = entities.iter().map(|x| x.x + x.size_x()).max().unwrap_or(0);
        let max_y = entities.iter().map(|x| x.y + x.size_y()).max().unwrap_or(0);

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
                    canvas.set(e.x+0,e.y+0, '┌');
                    canvas.set(e.x+1,e.y+0, '─');
                    canvas.set(e.x+2,e.y+0, '┐');
                    canvas.set(e.x+0,e.y+1, '│');
                    canvas.set(e.x+1,e.y+1, recipe.to_uppercase().chars().next().unwrap());
                    canvas.set(e.x+2,e.y+1, '│');
                    canvas.set(e.x+0,e.y+2, '└');
                    canvas.set(e.x+1,e.y+2, '─');
                    canvas.set(e.x+2,e.y+2, '┘');
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
                    canvas.set(e.x, e.y, symbol);
                }
                Function::Belt(d) => {
                    let symbol = match d {
                        Direction::Up => '⍐',
                        Direction::Down => '⍗',
                        Direction::Left => '⍇',
                        Direction::Right => '⍈',
                    };
                    canvas.set(e.x, e.y, symbol);
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
                    canvas.set(e.x, e.y, symbol);
                }
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

fn render_blueprint_ascii(entities: &[Entity]) {
    println!("{}", AsciiCanvas::build(entities).render());
}
fn render_blueprint_ingame(entities: &[Entity]) {
    use factorio_blueprint::{BlueprintCodec, Container};
    use factorio_blueprint::objects::*;
    use std::convert::TryInto;

    let container = Container::Blueprint(Blueprint {
        item: "blueprint".to_owned(),
        label: "very cool".to_owned(),
        label_color: None,
        version: 77310525440,
        schedules: vec![],
        icons: vec![Icon { index: OneBasedIndex::new(1).unwrap(), signal: SignalID { name: "electronic-circuit".to_owned(), type_: SignalIDType::Item } }],
        tiles: vec![],
        entities: entities.iter().enumerate().map(|(i, e)| {
            let mut underground_type = None;
            let mut recipe = None;
            let mut direction = None;
            let mut position = Position { x: (e.x as f64).try_into().unwrap(), y: (e.y as f64).try_into().unwrap() };
            let name = match e.function {
                Function::Assembler { recipe: ref r } => {
                    recipe = Some(r.clone());
                    position.x += 1.;
                    position.y += 1.;
                    "assembling-machine-2"
                }
                Function::Inserter { orientation, long_handed } => {
                    // reverse direction because the game thinks about these differently than we do
                    direction = Some(match orientation {
                        Direction::Up => Direction::Down,
                        Direction::Down => Direction::Up,
                        Direction::Left => Direction::Right,
                        Direction::Right => Direction::Left,
                    });
                    if long_handed {
                        "long-handed-inserter"
                    } else {
                        "inserter"
                    }
                }
                Function::Belt(d) => {
                    direction = Some(d);
                    "transport-belt"
                }
                Function::UndergroundBelt(d, down) => {
                    direction = Some(d);
                    underground_type = Some(if down { EntityType::Input } else { EntityType::Output });
                    "underground-belt"
                }
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
        }).collect(),
    });
    println!("{}", BlueprintCodec::encode_string(&container).unwrap());
}

#[derive(Debug, Clone)]
struct Recipe {
    ingredients: ItemSpec,
    results: ItemSpec,
    category: Category,
    crafting_time: f64,
}

fn read_recipes() -> Result<Vec<Recipe>> {
    let lua = Lua::new();
    lua.context(|c| {
        c.load(r#"
Importer = {}
Importer.__index = Importer

function Importer:create()
   local i = {}
   setmetatable(i,Importer)
   i.inner = {}
   return i
end

function Importer:extend(more)
   for _, v in ipairs(more) do
       table.insert(self.inner, v)
   end
end

data = Importer:create()
"#).exec()?;
        Ok(())
    })?;

    for file in std::fs::read_dir("../../windows/recipe").unwrap() {
        let file = file.unwrap();
        match file.path().extension() {
            Some(x) if x == "lua" => {
                println!("parsing {:?}", file.path());
                let code = std::fs::read_to_string(file.path()).unwrap();
                lua.context(|c| c.load(&code).exec())?;
            }
            _ => (),
        }
    }

    let mut recipes = Vec::new();

    lua.context::<_, Result<()>>(|c| {
        let data: Table = c.globals().get("data")?;
        let inner: Table = data.get("inner")?;
        for item in inner.sequence_values::<Table>() {
            let item = item?;
            if item.get::<_, String>("type")? == "recipe" {
                // ignore expensive mode
                let item = if item.contains_key("expensive")? {
                    item.get("normal")?
                } else {
                    item
                };

                let ingredients = normalize_item_spec(item.get("ingredients")?)?;
                let crafting_time = item.get("energy_required").unwrap_or(0.5);
                let results = match item.get("result") {
                    Ok(r) => vec![(r, item.get("result_count").unwrap_or(1))],
                    _ => normalize_item_spec(item.get("results")?)?,
                };
                
                let cat: String = item.get("category").unwrap_or("crafting".to_owned());
                let category = if cat == "crafting" || cat == "advanced-crafting" || cat == "crafting-with-fluid" {
                    Category::Assembler
                } else if cat == "centrifuging" {
                    Category::Centrifuge
                } else if cat == "chemistry" {
                    Category::ChemicalLab
                } else if cat == "oil-processing" {
                    Category::OilRefinery
                } else if cat == "rocket-building" {
                    Category::RocketSilo
                } else if cat == "smelting" {
                    Category::Furnace
                } else {
                    unimplemented!();
                };

                recipes.push(Recipe { results, crafting_time, category, ingredients });
            }
        }

        Ok(())
    })?;

    Ok(recipes)
}

#[derive(Debug, Clone)]
enum Category {
    Assembler,

    Centrifuge,
    ChemicalLab,
    OilRefinery,
    RocketSilo,
    Furnace,
}


type ItemSpec = Vec<(String, u32)>;
fn normalize_item_spec(table: Table) -> Result<ItemSpec> {
    let mut items = Vec::new();
    for item in table.sequence_values::<Table>() {
        let item = item?;
        let name;
        let amount;
        if item.contains_key("name")? {
            name = item.get("name")?;
            amount = item.get("amount")?;
        } else {
            name = item.get(1)?;
            amount = item.get(2)?;
        }
        items.push((name, amount));
    }
    Ok(items)
}

#[derive(Debug)]
struct ProductionGraph {
    output: String,
    per_second: f64,

    how_many: f64,
    building: &'static str,

    // has no input nodes if this node "produces" raw ores, i.e. is an external input
    inputs: Vec<ProductionGraph>,
}

fn kirkmcdonald(recipes: &[Recipe], desired: &str, desired_per_second: f64) -> ProductionGraph {
    if let Some(recipe) = recipes.iter().filter(|x| (x.results.len() == 1) && (x.results[0].0 == desired)).next() {
        let results_per_step = recipe.results[0].1 as f64;
        let step_duration = recipe.crafting_time;
        let results_per_second = results_per_step / step_duration;
        let how_many_concurrents = desired_per_second / results_per_second;

        let (how_many, building) = match recipe.category {
            Category::Assembler => (how_many_concurrents / 0.75, "assembler"),
            Category::Furnace => (how_many_concurrents / 2., "furnace"),
            _ => (-1., "<unimplemented>"),
        };

        let inputs = recipe.ingredients.iter().map(|&(ref d, amt)| kirkmcdonald(recipes, d, amt as f64 / results_per_step * desired_per_second)).collect();

        ProductionGraph {
            output: desired.to_owned(),
            per_second: desired_per_second,

            how_many,
            building,

            inputs,
        }
    } else {
        ProductionGraph {
            output: desired.to_owned(),
            per_second: desired_per_second,

            how_many: -1.,
            building: "<input>",

            inputs: vec![],
        }
    }
}

fn needed_assemblers<'a>(g: &'a ProductionGraph) -> Box<dyn Iterator<Item=&'a str> + 'a> {
    let upstream = g.inputs.iter().flat_map(needed_assemblers);
    if g.building == "assembler" {
        println!("i={}", g.inputs.len());
        Box::new(iter::repeat(&g.output as &str).take(g.how_many.ceil() as usize).chain(upstream))
    } else {
        Box::new(upstream)
    }
}

fn flatten() {

}

/*

# Types of wires:

* One-to-many: One gears assembler feeds many automation science pack assemblers
    * Trivial implementation: Belt connection
* Many-to-one
    * Trivial implementation: Belt connection
* Lane merge
    * Trivial implementation: L+R construction
 */

fn lee_pathfinder_new(entities: &mut Vec<Entity>, from: (i32, i32), to: (i32, i32)) {

let moveset = [
(Direction::Right, Translation::new(1, 0)),
(Direction::Down, Translation::new(0, 1)),
(Direction::Left, Translation::new(-1, 0)),
(Direction::Up, Translation::new(0, -1)),
];


let from = Point2::new(from.0, from.1);
let to = Point2::new(to.0, to.1);

    let path = mylee(entities, &moveset, from, to);

    let mut cursor = from;
    for step in path.unwrap() {
        let mov = moveset[step];

        entities.retain(|e| !e.overlaps(cursor.x, cursor.y)); // delete conflicting entities
        entities.push(Entity { x: cursor.x, y: cursor.y, function: Function::Belt(mov.0) });

        cursor = mov.1.transform_point(&cursor);
    }

}

use nalgebra::geometry::{Point2, Translation2};
type Point = Point2<i32>;
type Translation = Translation2<i32>;

fn mylee(entities: &[Entity], moveset: &[(Direction, Translation)], from: Point, to: Point) -> Option<Vec<usize>> {
struct Mazewalker {
    pos: Point,
    history: Vec<usize>,
}

let mut blocked_coords = Vec::new();

//let from = Point2::new(from.0, from.1);
//let to = Point2::new(to.0, to.1);


    // TODO: there's probably a much better algorithm based around some kind of cost heuristic
    let mut walkers = vec![Mazewalker { pos: from, history: Vec::new() }];
    while !walkers.is_empty() {
    println!("{} walkers {} blockers", walkers.len(), blocked_coords.len());
        for walker in std::mem::replace(&mut walkers, Vec::new()) {
    println!("{} vs {}", walker.pos, to);
            if walker.pos == to {
                return Some(walker.history);
            }
            
            for (i, &(_, trans)) in moveset.iter().enumerate() {
                let goto = trans.transform_point(&walker.pos);
                if entities.iter().any(|e| e.overlaps(goto.x, goto.y)) {
                    // blocked with existing entity
                    continue;
                }
                if blocked_coords.contains(&goto) {
                    // blocked with temporary entity
                    continue;
                }
                if goto.x.abs() > 30 || goto.y.abs() > 30 {
                    continue;
                }

                blocked_coords.push(goto); // could be a hashset

                let new_history = walker.history.iter().copied().chain(std::iter::once(i)).collect();
                walkers.push(Mazewalker { pos: goto, history: new_history });
            }
        }
    }
    None
}


fn lee_pathfinder(entities: &mut Vec<Entity>, from: (i32, i32), to: (i32, i32)) {
    use leemaze::{AllowedMoves2D, maze_directions2d};

    let max_x = entities.iter().map(|x| x.x + x.size_x()).max().unwrap_or(0) + 10;
    let max_y = entities.iter().map(|x| x.y + x.size_y()).max().unwrap_or(0) + 10;
    
    let mut rows = Vec::new();
    for y in -10..max_y {
        let mut row = Vec::new();
        for x in -10..max_x {
            row.push((x,y) != to && entities.iter().any(|e| e.overlaps(x, y)));
            if (x,y) == to {
                print!("T");
            } else if (x,y) == from {
                print!("F");
            } else if *row.last().unwrap() {
                print!("X");
            } else {
                print!(" ");
            }
        }
        rows.push(row);
        println!();
    }
    
    /*
    for row in &rows {
        for &x in row {
            if x {
                print!("X");
            } else {
                print!(" ");
            }
        }
        println!();
    }
    */
    
    let moveset = AllowedMoves2D {
        moves: vec![
            (1, 0),
            (0, 1),
            (-1, 0),
            (0, -1),

/*            // underground belts
            (6, 0),
            (0, 6),
            (-6, 0),
            (0, -6),*/
        ],
    };
    let path = maze_directions2d(&rows, &moveset, &((from.0 + 10) as usize, (from.1 + 10) as usize), &((to.0 + 10) as usize, (to.1 + 10) as usize));
    println!("{:?}", path);

    let moveset_dir = [
        Direction::Right,
        Direction::Down,
        Direction::Left,
        Direction::Up,
    ];

    let mut rows2 = rows.iter().map(|x| x.iter().map(|&b| if b { 'X' } else { ' ' }).collect::<Vec<_>>()).collect::<Vec<_>>();
    let mut path2 = vec![(from.0 + 10, from.1 + 10)];
    let mut path = path.expect("No path to target found");
    for &step in &path {
        let prev = path2.last().unwrap();
        let mov = moveset.moves[step];
        let next = (prev.0 + mov.0, prev.1 + mov.1);
        path2.push(next);
    }
    println!("{:?}", path2);
    
    for (i, &(x, y)) in path2.iter().enumerate() {
        let c = i.to_string().chars().last().unwrap();
        rows2[y as usize][x as usize] = c;
    }


    for row in &rows2 {
        for &x in row {
            print!("{}", x);
        }
        println!();
    }
    
    
    let mut undergrounded_path = Vec::new();
    let mut cut_iter = path.iter();
    while let Some(&current_direction) = cut_iter.next() {
        let is_continuation = match undergrounded_path.last() {
            Some(Ok(cd)) if *cd == current_direction => true,
            Some(Err((cd, gap))) if *cd == current_direction => true,
            _ => false,
        };
        let mut tail_length = cut_iter.clone().take_while(|&&d| d == current_direction).count();
        if is_continuation {
            tail_length += 1;
        }
        if tail_length > 2 {
            let gap = std::cmp::min(tail_length - 2, 4) as i32;

            for _ in 0..(gap + 1) {
                cut_iter.next().unwrap();
            }
            
        if !is_continuation {
                cut_iter.next().unwrap();
            undergrounded_path.push(Ok(current_direction)); // landing pad
            }
            undergrounded_path.push(Err((current_direction, gap))); // actual underground
        } else {
            undergrounded_path.push(Ok(current_direction));
        }
    }
    let mut cursor = from;
    for step in undergrounded_path {
        let (x, y) = cursor;
        entities.retain(|e| !e.overlaps(x, y)); // delete conflicting entities
        
        match step {
        Ok(step) => {
        entities.push(Entity { x, y, function: Function::Belt(moveset_dir[step]) });

        let mov = moveset.moves[step];
        cursor = (x + mov.0, y + mov.1);
        }
        Err((step, gap)) => {
        
        entities.push(Entity { x, y, function: Function::UndergroundBelt(moveset_dir[step], true) });
        let mov = moveset.moves[step];
        entities.push(Entity { x: x + mov.0*(gap+1), y: y + mov.1*(gap+1), function: Function::UndergroundBelt(moveset_dir[step], false) });

        cursor = (x + mov.0 * (gap+2), y + mov.1 * (gap+2));
        
        }
        }
    }
/*
    let mut cut_iter = 0;
    while cut_iter < path.len() {
        let current_direction = path[cut_iter];
        let run_length = path[cut_iter..].iter().take_while(|&&d| d == current_direction).count();
        if run_length > 3 {
            let gap = std::cmp::min(run_length - 3, 4);
            let gap_start = cut_iter + 2;
            path.drain(gap_start .. (gap_start + gap));
            
            cut_iter += 3;

            for _ in 0..gap {
            path.insert(gap_start, current_direction + 4);
            cut_iter += 1;
            }
        } else {
            cut_iter += 1;
        }
    }

    let mut cursor = from;
    for &step in &path {
        let (x, y) = cursor;
        if step >= 4 {
        let mov = moveset.moves[step - 4];
        cursor = (x + mov.0, y + mov.1);
        continue;
        }
    
        entities.retain(|e| !e.overlaps(x, y)); // delete conflicting entities
        entities.push(Entity { x, y, function: Function::Belt(moveset_dir[step]) });

        let mov = moveset.moves[step];
        cursor = (x + mov.0, y + mov.1);
    }
    */
}


fn gridrender_subtree(subtree: &ProductionGraph, grid_i: &mut i32, pcb: &mut Vec<Entity>, needed_wires: &mut Vec<((i32, i32), (i32, i32))>, gridsize: i32) -> Option<(Vec<(i32, i32)>, (i32, i32))> {
    if subtree.building == "assembler" {
    let mut upper_inputs = Vec::new();
    let mut our_inputs = Vec::new();
    
    for input in &subtree.inputs {
    match gridrender_subtree(input, grid_i, pcb, needed_wires, gridsize) {
    None => {
    // becomes an input instead
    our_inputs.push(None);
    }
    Some((ui, out)) => {
    upper_inputs.extend(ui);
    our_inputs.push(Some(out));
    }
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
		
		pcb.extend(vec![
		    Entity { x: startx + 2, y: starty + 0, function: Function::Assembler { recipe: subtree.output.clone() } },


		    // output belt
		    Entity { x: startx + 0, y: starty + 0, function: Function::Belt(Direction::Down) },
		    Entity { x: startx + 0, y: starty + 1, function: Function::Belt(Direction::Down) },
		    Entity { x: startx + 0, y: starty + 2, function: Function::Belt(Direction::Down) },
		    Entity { x: startx + 1, y: starty + 1, function: Function::Inserter { orientation: Direction::Left, long_handed: false } },

		    // input belt
		    Entity { x: startx + 6, y: starty + 0, function: Function::Belt(Direction::Up) },
		    Entity { x: startx + 6, y: starty + 1, function: Function::Belt(Direction::Up) },
		    Entity { x: startx + 6, y: starty + 2, function: Function::Belt(Direction::Up) },
		    Entity { x: startx + 5, y: starty + 0, function: Function::Inserter { orientation: Direction::Left, long_handed: false } },
		]);
		if let Some((sx, sy)) = prev {
		    needed_wires.push(((sx + 0, sy + 2), (startx + 0, starty + 0)));
		    needed_wires.push(((startx + 6, starty + 0), (sx + 6, sy + 2)));
		}
		
		if second_input_belt {
		pcb.extend(vec![
		    // input belt 2
		    Entity { x: startx + 7, y: starty + 0, function: Function::Belt(Direction::Up) },
		    Entity { x: startx + 7, y: starty + 1, function: Function::Belt(Direction::Up) },
		    Entity { x: startx + 7, y: starty + 2, function: Function::Belt(Direction::Up) },
		    Entity { x: startx + 5, y: starty + 1, function: Function::Inserter { orientation: Direction::Left, long_handed: true } },
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
                pcb.extend(vec![
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
                pcb.extend(vec![
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


fn main() {
    let recipes = read_recipes().unwrap();
    println!("Parsed {} recipes", recipes.len());
    
    println!("{:#?}", kirkmcdonald(&recipes, "logistic-science-pack", 0.75));

    let tree = kirkmcdonald(&recipes, "logistic-science-pack", 0.1);
    let needed_assemblers: Vec<_> = needed_assemblers(&tree).collect();
    println!("assemblers needed: {:?}", needed_assemblers);
    
    // very simple and stupid grid placer
    let gridsize = (needed_assemblers.len() as f64).sqrt().ceil() as i32;
    println!("gridsize={}", gridsize);
    
    let mut grid_i = 0;
    let mut pcb = Vec::new();
    let mut needed_wires = vec![];
    let (lins, lout) = gridrender_subtree(&tree, &mut grid_i, &mut pcb, &mut needed_wires, gridsize).unwrap();
    
    println!("rendering {} wires", needed_wires.len());
    for (from, to) in needed_wires.into_iter().rev() {
    render_blueprint_ascii(&pcb);
        lee_pathfinder(&mut pcb, from, to);
    }
    
    let gap_upper = 3;
    pcb.extend(vec![
        Entity { x: 0, y: -3 - gap_upper, function: Function::Belt(Direction::Up) },
        Entity { x: 0, y: -4 - gap_upper, function: Function::Belt(Direction::Up) },
    ]);
    for i in 0..lins.len() {
        pcb.push(Entity { x: i as i32 + 1, y: -3 - gap_upper, function: Function::Belt(Direction::Down) });
        pcb.push(Entity { x: i as i32 + 1, y: -4 - gap_upper, function: Function::Belt(Direction::Down) });
    }

    render_blueprint_ascii(&pcb);
    lee_pathfinder(&mut pcb, lout, (0, -3 - gap_upper));
    for (i, lin) in lins.into_iter().enumerate().rev() {
    render_blueprint_ascii(&pcb);
        lee_pathfinder(&mut pcb, (i as i32 + 1, -3 - gap_upper), lin);
    }

    /*
//    lee_pathfinder(&pcb, (10, 2), (25, 10));
    lee_pathfinder(&mut pcb, (30, 12), (21, 12));
    render_blueprint_ascii(&pcb);
    lee_pathfinder(&mut pcb, (21, 10), (6, 12));
    render_blueprint_ascii(&pcb);
    lee_pathfinder(&mut pcb, (6, 10), (36, 2));
    render_blueprint_ascii(&pcb);
    lee_pathfinder(&mut pcb, (36, 0), (21, 2));
    render_blueprint_ascii(&pcb);
    lee_pathfinder(&mut pcb, (21, 0), (6, 2));

    render_blueprint_ascii(&pcb);
    lee_pathfinder(&mut pcb, (0, 15), (7, 12));
    render_blueprint_ascii(&pcb);
    lee_pathfinder(&mut pcb, (0, 16), (7, 2));
    render_blueprint_ascii(&pcb);
    lee_pathfinder(&mut pcb, (0, 17), (22, 2));
    render_blueprint_ascii(&pcb);
    lee_pathfinder(&mut pcb, (0, 18), (37, 2));
    render_blueprint_ascii(&pcb);
    lee_pathfinder(&mut pcb, (0, 19), (22, 12));

    render_blueprint_ascii(&pcb);
    lee_pathfinder(&mut pcb, (0, 20), (36, 12));

//    lee_pathfinder(&pcb, (0, 2), (38, 10));
*/

    render_blueprint_ascii(&pcb);
    render_blueprint_ingame(&pcb);
    /*
    render_blueprint_ascii(vec![
        Entity { x: 2, y: 0, function: Function::Assembler { recipe: "gears".to_owned() } },
        Entity { x: 1, y: 1, function: Function::Inserter(Direction::Left) },
        Entity { x: 0, y: 0, function: Function::Belt(Direction::Down) },
        Entity { x: 0, y: 1, function: Function::Belt(Direction::Down) },
        Entity { x: 0, y: 2, function: Function::Belt(Direction::Down) },
        Entity { x: 0, y: 3, function: Function::Belt(Direction::Down) },
    ]);
    */
}
