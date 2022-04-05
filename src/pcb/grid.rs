use ndarray::{s, Array2};
use fehler::throws;
use std::borrow::Borrow;
use std::slice::Iter;
use std::iter::FilterMap;

use super::*;

#[derive(Debug, Clone)]
pub struct GridPcb {
    entities: Vec<Option<Entity>>,

    grid_origin: Vector,
    grid: Array2<usize>, // contains index in enities + 1 (zero is none)

    entity_rect: CachedEntityRect,
}

impl Default for GridPcb {
    fn default() -> GridPcb {
        GridPcb {
            entities: Vec::new(),

            grid_origin: Vector::new(0, 0),
            grid: Array2::zeros((0, 0)),

            entity_rect: Default::default(),
        }
    }
}

impl GridPcb {
    fn resize_grid(&mut self) {
        let entity_rect = self.entity_rect();

        let min_vec = entity_rect.a.coords;
        let max_vec = entity_rect.b.coords;
        let used_rect = max_vec - min_vec;
        let desired_space = used_rect * 2;

        let old_shape = self.grid.shape();
        let old_space = Vector::new(old_shape[0] as i32, old_shape[1] as i32);
        assert!(old_space != desired_space); // make sure we actually DO something

        let mut newgrid = Array2::zeros((desired_space.x as usize, desired_space.y as usize));
        let new_origin = min_vec - (used_rect / 2);
        let old_origin = self.grid_origin;
        let transform = -(new_origin - old_origin);
        let end_transform = transform + old_space;

        //println!("{} {} {} {} {:?} {} {} {} {}", min_vec, max_vec, used_rect, desired_space, old_shape, new_origin, old_origin, transform, end_transform);
        if transform != end_transform {
            newgrid.slice_mut(s![transform[0]..end_transform[0], transform[1]..end_transform[1]]).assign(&self.grid);
        }

        self.grid_origin = new_origin;
        self.grid = newgrid;
    }

    #[throws(as Option)]
    fn place_entity_on_grid(&mut self, entity: &Entity, index: usize) {
        for tile in entity_tiles(&entity, self.grid_origin) {
            let tile = self.grid.get_mut((tile.x as usize, tile.y as usize))?;
            let entities = &self.entities;
            assert!((*tile).checked_sub(1).and_then(|i| entities.get(i).and_then(|e| e.as_ref())).is_none(), "Conflicting entities");
            *tile = index + 1;
        }
    }
}
impl<'a> PcbRef<'a> for GridPcb {
    type EntityIter = FilterMap<Iter<'a, Option<Entity>>, fn(&Option<Entity>) -> Option<&Entity>>;
    fn entities(&'a self) -> Self::EntityIter {
        self.entities.iter().filter_map(Option::as_ref)
    }

    fn entity_rect(&'a self) -> Rect {
        self.entity_rect.rect()
    }
}

impl Pcb for GridPcb {
    fn add(&mut self, entity: impl Borrow<Entity>) {
        let entity = entity.borrow();
        let index = self.entities.len();
        self.entity_rect.update(entity);

        while self.place_entity_on_grid(entity, index).is_none() {
            self.resize_grid();
        }

        // try to place entity first so that when we retry the placement code it won't get mad
        // about existing tiles of our entity (which read as None thanks to this)
        self.entities.push(Some(entity.clone()));
    }

    fn remove_at(&mut self, point: Point) {
        let grid_idx = point - self.grid_origin;
        let prev_entity = self.grid.get((grid_idx.x as usize, grid_idx.y as usize))
            .and_then(|i| i.checked_sub(1))
            .and_then(|i| self.entities[i].take());

        if let Some(entity) = prev_entity {
            for tile in entity_tiles(&entity, self.grid_origin) {
                *self.grid.get_mut((tile.x as usize, tile.y as usize)).unwrap() = 0;
            }
        }
    }
    fn entity_at(&self, point: Point) -> Option<&Entity> {
        let grid_idx = point - self.grid_origin;
        let idx = self.grid.get((grid_idx.x as usize, grid_idx.y as usize))?;
        let idx = idx.checked_sub(1)?;
        self.entities.get(idx)?.as_ref()
    }
}


#[cfg(test)]
mod test {
    use super::*;

    fn pcb_invariant(pcb: &GridPcb) {
        let s = pcb.grid.shape();
        for x in 0..s[0] {
            for y in 0..s[1] {
                let v = Vector::new(x as i32, y as i32);
                let gp = pcb.grid_origin + v;
                let confl = pcb.entities().enumerate().filter(|(_, e)| e.overlaps(Point { coords: gp })).map(|(i, _)| i).next();
                let idx = pcb.grid[(x as usize, y as usize)];
                assert_eq!(confl.map(|i| i + 1).unwrap_or(0), idx);
            }
        }
    }

    #[test]
    fn pcb_works() {
        let mut pcb = GridPcb::default();
        pcb_invariant(&pcb);

        pcb.add(&Entity { location: Point::new(42, 69), function: Function::Belt(Direction::Up) });
        dbg!(&pcb);
        pcb_invariant(&pcb);

        pcb.add(&Entity { location: Point::new(0, 0), function: Function::Belt(Direction::Up) });
        dbg!(&pcb);
        pcb_invariant(&pcb);

        pcb.add(&Entity { location: Point::new(13, 13), function: Function::Belt(Direction::Up) });
        dbg!(&pcb);
        pcb_invariant(&pcb);
    }
}

