use fnv::FnvHashMap;
use std::borrow::Borrow;
use std::slice::Iter;
use std::iter::FilterMap;

use super::*;


#[derive(Debug, Clone, Default)]
pub struct HashmapPcb {
    entities: Vec<Option<Entity>>,
    grid: FnvHashMap<Point, usize>,
}

impl<'a> PcbRef<'a> for HashmapPcb {
    type EntityIter = FilterMap<Iter<'a, Option<Entity>>, fn(&Option<Entity>) -> Option<&Entity>>;
    fn entities(&'a self) -> Self::EntityIter {
        self.entities.iter().filter_map(Option::as_ref)
    }
}

impl Pcb for HashmapPcb {
    fn add(&mut self, entity: impl Borrow<Entity>) {
        let entity = entity.borrow();
        let index = self.entities.len();

        self.entities.push(Some(entity.clone()));

        for tile in entity_tiles(entity, Vector::zeros()) {
            let prev = self.grid.insert(tile, index);
            assert!(prev.is_none());
        }
    }

    fn remove_at(&mut self, point: Point) {
        let prev_entity = self.grid.remove(&point).and_then(|i| self.entities[i].take());
        if let Some(e) = prev_entity {
            for tile in entity_tiles(&e, Vector::zeros()) {
                self.grid.remove(&tile);
            }
        }
    }

    fn entity_at(&self, point: Point) -> Option<&Entity> {
        self.grid.get(&point).and_then(|&i| self.entities[i].as_ref())
    }
}

