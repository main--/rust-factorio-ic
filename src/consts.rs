use crate::Rational;

pub struct Constants {
    pub inserter_capacity_bonus: i32,
    pub max_belts: BeltType, // currently we don't understand faster belts
}
impl Default for Constants {
    fn default() -> Self {
        Self {
            inserter_capacity_bonus: 7,
            max_belts: BeltType::Normal,
        }
    }
}
impl Constants {
    pub fn basic_inserter_items_per_second(&self) -> Rational {
        // TODO: understand faster belts
        match self.inserter_capacity_bonus {
            0 => Rational::new(94, 100),
            2 => Rational::new(167, 100),
            7 => Rational::new(250, 100),
            _ => todo!(),
        }
    }
    pub fn long_inserter_items_per_second(&self) -> Rational {
        // TODO: understand faster belts
        match self.inserter_capacity_bonus {
            0 => Rational::new(118, 100),
            2 => Rational::new(220, 100),
            7 => Rational::new(321, 100),
            _ => todo!(),
        }
    }
    pub fn fast_inserter_items_per_second(&self) -> Rational {
        // TODO: understand faster belts
        match self.inserter_capacity_bonus {
            0 => Rational::new(250, 100),
            2 => Rational::new(450, 100),
            7 => Rational::new(643, 100),
            _ => todo!(),
        }
    }
    pub fn stack_inserter_items_per_second(&self) -> Rational {
        // TODO: understand faster belts
        match self.inserter_capacity_bonus {
            0 => Rational::new(450, 100),
            2 => Rational::new(750, 100),
            7 => Rational::new(750, 100), // this is probably wrong
            _ => todo!(),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeltType {
    Normal,
    Fast,
    Express,
}
impl BeltType {
    pub fn lane_items_per_second(&self) -> Rational {
        match self {
            BeltType::Normal => Rational::new(15, 2),
            BeltType::Fast => Rational::new(30, 2),
            BeltType::Express => Rational::new(45, 2),
        }
    }
}
