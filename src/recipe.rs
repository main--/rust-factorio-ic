use std::path::Path;

use rlua::{Lua, Result, Table};

use crate::pcb::WireKind;

#[derive(Debug, Clone)]
pub struct Recipe {
    pub ingredients: ItemSpec,
    pub results: ItemSpec,
    pub category: Category,
    pub crafting_time: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Category {
    Assembler,

    // not yet implemented
    Centrifuge,
    ChemicalLab,
    OilRefinery,
    RocketSilo,
    Furnace,
}

pub type ItemSpec = Vec<Ingredient>;
#[derive(Debug, Clone)]
pub struct Ingredient {
    pub name: String,
    pub amount: u32,
    pub kind: WireKind,
}

pub fn extract_recipes(path: impl AsRef<Path>) -> Result<Vec<Recipe>> {
    let lua = Lua::new();
    lua.context(|c| {
        c.load(
            r#"
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
"#,
        )
            .exec()?;
        Ok(())
    })?;

    for file in std::fs::read_dir(path).unwrap() {
        let file = file.unwrap();
        match file.path().extension() {
            Some(x) if x == "lua" => {
                println!("parsing {:?}", file.path());
                let code = std::fs::read_to_string(file.path()).unwrap();
                lua.context(|c| c.load(&code).exec())?;
            },
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
                let cat: String = item.get("category").unwrap_or("crafting".to_owned());
                // ignore expensive mode
                let item = if item.contains_key("expensive")? { item.get("normal")? } else { item };

                let ingredients = normalize_item_spec(item.get("ingredients")?)?;
                let crafting_time = item.get("energy_required").unwrap_or(0.5);
                let results = match item.get("result") {
                    Ok(r) => vec![Ingredient { name: r, amount: item.get("result_count").unwrap_or(1), kind: WireKind::Belt }],
                    _ => normalize_item_spec(item.get("results")?)?,
                };

                let category = if cat == "crafting"
                    || cat == "advanced-crafting"
                    || cat == "crafting-with-fluid"
                {
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

fn normalize_item_spec(table: Table) -> Result<ItemSpec> {
    let mut items = Vec::new();
    for item in table.sequence_values::<Table>() {
        let item = item?;
        let name;
        let amount;
        let kind = match item.get::<_, String>("type") {
            Ok(s) if s == "fluid" => WireKind::Pipe,
            _ => WireKind::Belt,
        };
        if item.contains_key("name")? {
            name = item.get("name")?;
            amount = item.get("amount")?;
        } else {
            name = item.get(1)?;
            amount = item.get(2)?;
        }
        items.push(Ingredient { name, amount, kind });
    }
    Ok(items)
}
