use std::iter::{self, FromIterator};
use rlua::{Table, Lua, Result};

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
}
struct Entity {
    x: i32,
    y: i32,
    function: Function,
}
impl Entity {
    fn size_x(&self) -> i32 {
        match self.function {
            Function::Belt(_) | Function::Inserter { .. } => 1,
            Function::Assembler { .. } => 3,
        }
    }
    fn size_y(&self) -> i32 {
        self.size_x() // currently everything is quadratic
    }
}

struct AsciiCanvas {
    offset_x: i32,
    offset_y: i32,
    canvas: Vec<Vec<char>>,
}
impl AsciiCanvas {
    fn build(entities: Vec<Entity>) -> Self {
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
                Function::Assembler { recipe } => {
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

fn render_blueprint_ascii(entities: Vec<Entity>) {
    println!("{}", AsciiCanvas::build(entities).render());
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
        Box::new(iter::repeat(&g.output as &str).take(g.how_many.ceil() as usize).chain(upstream))
    } else {
        Box::new(upstream)
    }
}

fn flatten() {

}

fn main() {
    let recipes = read_recipes().unwrap();
    println!("Parsed {} recipes", recipes.len());
    
    println!("{:#?}", kirkmcdonald(&recipes, "logistic-science-pack", 0.75));

    let tree = kirkmcdonald(&recipes, "logistic-science-pack", 0.75);
    let needed_assemblers: Vec<_> = needed_assemblers(&tree).collect();
    println!("assemblers needed: {:?}", needed_assemblers);
    
    // very simple and stupid grid placer
    let gridsize = (needed_assemblers.len() as f64).sqrt().ceil() as i32;
    println!("gridsize={}", gridsize);
    
    let mut pcb = Vec::new();
    for (i, &a) in needed_assemblers.iter().enumerate() {
        let i = i as i32;
        let grid_x = i % gridsize;
        let grid_y = i / gridsize;
        
        let cell_size_x = 15;
        let cell_size_y = 10;
        
        let startx = cell_size_x * grid_x;
        let starty = cell_size_y * grid_y;
        
        pcb.extend(vec![
            // output belt
            Entity { x: startx + 0, y: starty + 0, function: Function::Belt(Direction::Down) },
            Entity { x: startx + 0, y: starty + 1, function: Function::Belt(Direction::Down) },
            Entity { x: startx + 0, y: starty + 2, function: Function::Belt(Direction::Down) },

            // input belt
            Entity { x: startx + 6, y: starty + 0, function: Function::Belt(Direction::Up) },
            Entity { x: startx + 6, y: starty + 1, function: Function::Belt(Direction::Up) },
            Entity { x: startx + 6, y: starty + 2, function: Function::Belt(Direction::Up) },
            // input belt 2
            Entity { x: startx + 7, y: starty + 0, function: Function::Belt(Direction::Up) },
            Entity { x: startx + 7, y: starty + 1, function: Function::Belt(Direction::Up) },
            Entity { x: startx + 7, y: starty + 2, function: Function::Belt(Direction::Up) },

            Entity { x: startx + 2, y: starty + 0, function: Function::Assembler { recipe: a.to_owned() } },
            Entity { x: startx + 1, y: starty + 1, function: Function::Inserter { orientation: Direction::Left, long_handed: false } },
            Entity { x: startx + 5, y: starty + 0, function: Function::Inserter { orientation: Direction::Left, long_handed: false } },
            Entity { x: startx + 5, y: starty + 1, function: Function::Inserter { orientation: Direction::Left, long_handed: true } },
        ]);
    }
    

    render_blueprint_ascii(pcb);
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
