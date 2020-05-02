use ndarray::{s, Array2};
use nalgebra::geometry::Point2;
use nalgebra::base::Vector2;
use fehler::throws;
use std::borrow::Borrow;

pub type Point = Point2<i32>;
pub type Vector = Vector2<i32>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}
pub const ALL_DIRECTIONS: [Direction; 4] = [Direction::Right, Direction::Left, Direction::Down, Direction::Up];
impl Direction {
    pub fn to_vector(&self) -> Vector {
        match self {
            Direction::Up => Vector::new(0, -1),
            Direction::Down => Vector::new(0, 1),
            Direction::Left => Vector::new(-1, 0),
            Direction::Right => Vector::new(1, 0),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Function {
    Assembler { recipe: String },
    Inserter { orientation: Direction, long_handed: bool },
    Belt(Direction),
    UndergroundBelt(Direction, bool),
}
#[derive(Debug, Clone)]
pub struct Entity {
    pub location: Point,
    pub function: Function,
}
impl Entity {
    pub fn size_x(&self) -> i32 {
        match self.function {
            Function::Belt(_) | Function::UndergroundBelt(_, _) | Function::Inserter { .. } => 1,
            Function::Assembler { .. } => 3,
        }
    }

    pub fn size_y(&self) -> i32 {
        self.size_x() // currently everything is quadratic
    }

    pub fn overlaps(&self, x: i32, y: i32) -> bool {
        (self.location.x <= x)
            && (self.location.x + self.size_x() > x)
            && (self.location.y <= y)
            && (self.location.y + self.size_y() > y)
    }
}

/// `a` must be top left and `b` must be bottom right
pub struct Rect {
    pub a: Point,
    pub b: Point,
}

impl Rect {
    pub fn contains(&self, point: Point) -> bool {
        self.a.x <= point.x && point.x < self.b.x
        && self.a.y <= point.y && point.y < self.b.y
    }
}

pub type NeededWires = Vec<((i32, i32), (i32, i32))>;

#[derive(Debug, Clone)]
pub struct Pcb {
    entities: Vec<Option<Entity>>,

    grid_origin: Vector,
    grid: Array2<usize>, // contains index in enities + 1 (zero is none)
}

impl Pcb {
    pub fn new() -> Pcb {
        Pcb {
            entities: Vec::new(),

            grid_origin: Vector::new(0, 0),
            grid: Array2::zeros((0, 0)),
        }
    }
    pub fn resize_grid(&mut self) {
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
    pub fn add_all<I>(&mut self, iter: I)
        where I: IntoIterator, I::Item: Borrow<Entity> {
        // TODO: there is a more efficient impl here
        for e in iter {
            self.add(e);
        }
    }
    pub fn replace(&mut self, entity: impl Borrow<Entity>) {
        self.remove_at((entity.borrow().location.x, entity.borrow().location.y));
        self.add(entity);
    }
    pub fn add(&mut self, entity: impl Borrow<Entity>) {
        let entity = entity.borrow();
        let index = self.entities.len();
        self.entities.push(Some(entity.clone()));

        while self.place_entity_on_grid(entity, index).is_none() {
            self.resize_grid();
        }
    }

    #[throws(as Option)]
    fn place_entity_on_grid(&mut self, entity: &Entity, index: usize) {
        let tiles = (0..entity.size_x()).flat_map(|x| (0..entity.size_y()).map(move |y| Point::new(x, y)));
        let tiles_origin = Vector::new(entity.location.x, entity.location.y) - self.grid_origin;
        let tiles = tiles.map(|t| t + tiles_origin);
        for tile in tiles {
            let tile = self.grid.get_mut((tile.x as usize, tile.y as usize))?;
            *tile = index + 1;
        }
    }
    pub fn entities<'a>(&'a self) -> impl Iterator<Item=&'a Entity> + Clone {
        self.entities.iter().filter_map(|o| o.as_ref())
    }

    pub fn remove_at(&mut self, point: (i32, i32)) {
        let grid_idx = Vector::new(point.0, point.1) - self.grid_origin;
        if let Some(i) = self.grid.get((grid_idx.x as usize, grid_idx.y as usize)).and_then(|i| i.checked_sub(1)) {
            // TODO: this leaves the index values in the grid dangling, which is not a problem but also
            //       prevents us from ever re-using the gaps
            self.entities[i] = None;
        }
    }
    pub fn is_empty(&self, point: (i32, i32)) -> bool {
        let grid_idx = Vector::new(point.0, point.1) - self.grid_origin;
        self.grid.get((grid_idx.x as usize, grid_idx.y as usize)).and_then(|i| i.checked_sub(1)).is_none()
//        !self.entities().any(|e| e.overlaps(point.0, point.1))
    }
    pub fn entity_rect(&self) -> Rect {
        if self.entities.is_empty() {
            return Rect {
                a: Point::new(0, 0),
                b: Point::new(0, 0),
            };
        }
        let mut min_x = i32::MAX;
        let mut max_x = i32::MIN;
        let mut min_y = i32::MAX;
        let mut max_y = i32::MIN;
        for entity in self.entities() {
            min_x = min_x.min(entity.location.x);
            max_x = max_x.max(entity.location.x + entity.size_x());
            min_y = min_y.min(entity.location.y);
            max_y = max_y.max(entity.location.y + entity.size_y());
        }
        Rect {
            a: Point::new(min_x, min_y),
            b: Point::new(max_x, max_y),
        }
    }
    pub fn grid_capacity(&self) -> Rect {
        let (size_x, size_y) = self.grid.dim();
        Rect {
            a: Point { coords: self.grid_origin },
            b: Point::new(size_x as i32 - self.grid_origin.x, size_y as i32 - self.grid_origin.y),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn pcb_invariant(pcb: &Pcb) {
        let s = pcb.grid.shape();
        for x in 0..s[0] {
            for y in 0..s[1] {
                let v = Vector::new(x as i32, y as i32);
                let gp = pcb.grid_origin + v;
                let confl = pcb.entities().enumerate().filter(|(_, e)| e.overlaps(gp.x, gp.y)).map(|(i, _)| i).next();
                let idx = pcb.grid[(x as usize, y as usize)];
                assert_eq!(confl.map(|i| i + 1).unwrap_or(0), idx);
            }
        }
    }

    #[test]
    fn pcb_works() {
        let mut pcb = Pcb::new();
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
