use fnv::FnvHashMap;
use std::borrow::Borrow;

use super::*;


#[derive(Debug, Clone, Default)]
pub struct Pcb {
    entities: Vec<Entity>,
}

impl PcbImpl for Pcb {
    fn add(&mut self, entity: impl Borrow<Entity>) {
        let entity = entity.borrow();

        assert!(self.entity_at(entity.location).is_none());
        self.entities.push(entity.clone());
    }

    fn remove_at(&mut self, point: Point) {
        self.entities.retain(|e| e.location != point);
    }

    fn entity_at(&self, point: Point) -> Option<&Entity> {
        self.entities.iter().filter(|e| e.location == point).next()
    }
}

impl Pcb {
    pub fn entities<'a>(&'a self) -> impl Iterator<Item=&'a Entity> + Clone {
        self.entities.iter()
    }
}

