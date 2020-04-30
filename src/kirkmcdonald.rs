use std::iter;

use crate::recipe::{Category, Recipe};

#[derive(Debug)]
pub struct ProductionGraph {
    pub output: String,
    pub per_second: f64,

    pub how_many: f64,
    pub building: &'static str,

    // has no input nodes if this node "produces" raw ores, i.e. is an external input
    pub inputs: Vec<ProductionGraph>,
}

pub fn kirkmcdonald(recipes: &[Recipe], desired: &str, desired_per_second: f64) -> ProductionGraph {
    if let Some(recipe) =
    recipes.iter().filter(|x| (x.results.len() == 1) && (x.results[0].0 == desired)).next()
    {
        let results_per_step = recipe.results[0].1 as f64;
        let step_duration = recipe.crafting_time;
        let results_per_second = results_per_step / step_duration;
        let how_many_concurrents = desired_per_second / results_per_second;

        let (how_many, building) = match recipe.category {
            Category::Assembler => (how_many_concurrents / 0.75, "assembler"),
            Category::Furnace => (how_many_concurrents / 2., "furnace"),
            _ => (-1., "<unimplemented>"),
        };

        let inputs = recipe
            .ingredients
            .iter()
            .map(|&(ref d, amt)| {
                kirkmcdonald(recipes, d, amt as f64 / results_per_step * desired_per_second)
            })
            .collect();

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

pub fn needed_assemblers<'a>(g: &'a ProductionGraph) -> Box<dyn Iterator<Item = &'a str> + 'a> {
    let upstream = g.inputs.iter().flat_map(needed_assemblers);
    if g.building == "assembler" {
        println!("i={}", g.inputs.len());
        Box::new(iter::repeat(&g.output as &str).take(g.how_many.ceil() as usize).chain(upstream))
    } else {
        Box::new(upstream)
    }
}

