use crate::Rational;
use crate::pcb::WireKind;
use crate::recipe::{Category, Recipe, Ingredient};

#[derive(Debug)]
pub struct ProductionGraph {
    pub output: String,
    pub output_kind: WireKind,
    pub per_second: Rational,

    pub how_many: Rational,
    pub building: Option<Category>,

    // has no input nodes if this node "produces" raw ores, i.e. is an external input
    pub inputs: Vec<ProductionGraph>,
}

pub fn kirkmcdonald(recipes: &[Recipe], desired: &str, desired_per_second: Rational, output_kind: &WireKind) -> ProductionGraph {
    if output_kind != &WireKind::Belt {
        // right now we just blindly assume that we can't produce pipe outputs ever
        return ProductionGraph {
            output: desired.to_owned(),
            output_kind: output_kind.clone(),
            per_second: desired_per_second,

            how_many: Rational::from(-1),
            building: None,

            inputs: vec![],
        };
    }

    if let Some(recipe) = recipes.iter().filter(|x| (x.results.len() == 1) && (x.results[0].name == desired)).next() {
        let results_per_step = recipe.results[0].amount;
        let step_duration = Rational::approximate_float(recipe.crafting_time).unwrap();
        let results_per_second = results_per_step / step_duration;
        let how_many_concurrents = desired_per_second / results_per_second;

        let building_base_speed = match recipe.category {
            Category::Assembler => Rational::new(3, 4),
            Category::Furnace => Rational::from(2),
            _ => Rational::from(-1), // unimplemented
        };
        let how_many = how_many_concurrents / building_base_speed;

        let inputs = recipe
            .ingredients
            .iter()
            .map(|&Ingredient { ref name, amount, ref kind }| {
                kirkmcdonald(recipes, name, amount / results_per_step * desired_per_second, kind)
            })
            .collect();

        ProductionGraph {
            output: desired.to_owned(),
            output_kind: output_kind.clone(),
            per_second: desired_per_second,

            how_many,
            building: Some(recipe.category),

            inputs,
        }
    } else {
        ProductionGraph {
            output: desired.to_owned(),
            output_kind: output_kind.clone(),
            per_second: desired_per_second,

            how_many: Rational::from(-1),
            building: None,

            inputs: vec![],
        }
    }
}

