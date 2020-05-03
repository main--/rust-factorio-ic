use fnv::FnvHashMap;
use std::borrow::Borrow;

use super::*;


#[derive(Debug, Clone, Default)]
pub struct Pcb {
    entities: Vec<Option<Entity>>,
    grid: FnvHashMap<Point, usize>,
}

impl PcbImpl for Pcb {
    fn add(&mut self, entity: impl Borrow<Entity>) {
        let entity = entity.borrow();
        let index = self.entities.len();

        self.entities.push(Some(entity.clone()));

        for tile in entity_tiles(entity) {
            let prev = self.grid.insert(tile, index);
            assert!(prev.is_none());
        }
    }

    fn remove_at(&mut self, point: Point) {
        if let Some(i) = self.grid.remove(&point) {
            if let Some(e) = std::mem::replace(&mut self.entities[i], None) {
                for tile in entity_tiles(&e) { self.grid.remove(&tile); }
            }
        }
    }

    fn entity_at(&self, point: Point) -> Option<&Entity> {
        self.grid.get(&point).and_then(|&i| self.entities[i].as_ref())
    }
}

impl Pcb {
    pub fn entities<'a>(&'a self) -> impl Iterator<Item=&'a Entity> + Clone {
        self.entities.iter().filter_map(|o| o.as_ref())
    }
}

