use fnv::FnvHashMap;
use std::borrow::Borrow;
use std::slice::Iter;

use super::*;


#[derive(Debug, Clone, Default)]
pub struct NaivePcb {
    entities: Vec<Entity>,
}

impl<'a> Pcb<'a> for NaivePcb {
    type EntityIter = Iter<'a, Entity>;
    fn entities(&'a self) -> Self::EntityIter {
        self.entities.iter()
    }

    fn add(&mut self, entity: impl Borrow<Entity>) {
        let entity = entity.borrow();

        assert!(self.entity_at(entity.location).is_none());
        self.entities.push(entity.clone());
    }

    fn remove_at(&mut self, point: Point) {
        self.entities.retain(|e| !e.overlaps(point));
    }

    fn entity_at(&self, point: Point) -> Option<&Entity> {
        self.entities.iter().filter(|e| e.overlaps(point)).next()
    }
}

