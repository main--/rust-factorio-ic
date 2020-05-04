use std::iter;

use crate::recipe::{Category, Recipe};

#[derive(Debug)]
pub struct ProductionGraph {
    pub output: String,
    pub per_second: f64,

    pub how_many: f64,
    pub building: Option<Category>,

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

        let building_base_speed = match recipe.category {
            Category::Assembler => 0.75,
            Category::Furnace => 2.,
            _ => -1., // unimplemented
        };
        let how_many = how_many_concurrents / building_base_speed;

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
            building: Some(recipe.category),

            inputs,
        }
    } else {
        ProductionGraph {
            output: desired.to_owned(),
            per_second: desired_per_second,

            how_many: -1.,
            building: None,

            inputs: vec![],
        }
    }
}

