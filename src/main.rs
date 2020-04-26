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
    Inserter(Direction),
    Belt(Direction),
}
struct Entity {
    x: usize,
    y: usize,
    function: Function,
}
impl Entity {
    fn size_x(&self) -> usize {
        match self.function {
            Function::Belt(_) | Function::Inserter(_) => 1,
            Function::Assembler { .. } => 3,
        }
    }
    fn size_y(&self) -> usize {
        self.size_x() // currently everything is quadratic
    }
}


fn render_blueprint_ascii(entities: Vec<Entity>) {
    let size_x = entities.iter().map(|x| x.x + x.size_x()).max().unwrap_or(0);
    let size_y = entities.iter().map(|x| x.y + x.size_y()).max().unwrap_or(0);
    
    let canvas_row: Vec<char> = iter::repeat(' ').take(size_x).collect();
    let mut canvas: Vec<_> = iter::repeat(canvas_row).take(size_y).collect();
    
    for e in entities {
        match e.function {
            Function::Assembler { recipe } => {
                canvas[e.y+0][e.x+0] = '┌';
                canvas[e.y+0][e.x+1] = '─';
                canvas[e.y+0][e.x+2] = '┐';
                canvas[e.y+1][e.x+0] = '│';
                canvas[e.y+1][e.x+1] = recipe.to_uppercase().chars().next().unwrap();
                canvas[e.y+1][e.x+2] = '│';
                canvas[e.y+2][e.x+0] = '└';
                canvas[e.y+2][e.x+1] = '─';
                canvas[e.y+2][e.x+2] = '┘';
            }
            Function::Inserter(d) => {
                let symbol = match d {
                    Direction::Up => '↑',
                    Direction::Down => '↓',
                    Direction::Left => '←',
                    Direction::Right => '→',
                };
                canvas[e.y][e.x] = symbol;
            }
            Function::Belt(d) => {
                let symbol = match d {
                    Direction::Up => '⍐',
                    Direction::Down => '⍗',
                    Direction::Left => '⍇',
                    Direction::Right => '⍈',
//                    Direction::Up | Direction::Down => '║',
//                    Direction::Left | Direction::Right => '═',
                };
                canvas[e.y][e.x] = symbol;
            },
        }
    }

    for row in canvas {
        println!("{}", String::from_iter(row));
    }
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
            _ => unimplemented!(),
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


fn main() {
    let recipes = read_recipes().unwrap();
    println!("Parsed {} recipes", recipes.len());
    
    println!("{:#?}", kirkmcdonald(&recipes, "logistic-science-pack", 0.75));

    render_blueprint_ascii(vec![
        Entity { x: 2, y: 0, function: Function::Assembler { recipe: "gears".to_owned() } },
        Entity { x: 1, y: 1, function: Function::Inserter(Direction::Left) },
        Entity { x: 0, y: 0, function: Function::Belt(Direction::Down) },
        Entity { x: 0, y: 1, function: Function::Belt(Direction::Down) },
        Entity { x: 0, y: 2, function: Function::Belt(Direction::Down) },
        Entity { x: 0, y: 3, function: Function::Belt(Direction::Down) },
    ]);
}
