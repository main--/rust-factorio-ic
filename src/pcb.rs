use ndarray::{s, Array2};
use nalgebra::geometry::Point2;
use nalgebra::base::Vector2;
use fnv::FnvHashMap;
use fehler::throws;
use std::borrow::Borrow;

pub type Point = Point2<i32>;
pub type Vector = Vector2<i32>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    pub fn is_same_axis(&self, other: Direction) -> bool {
        match self {
            Direction::Up | Direction::Down => other == Direction::Up || other == Direction::Down,
            Direction::Left | Direction::Right => other == Direction::Left || other == Direction::Right,
        }
    }
    pub fn opposite_direction(&self) -> Direction {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Function {
    Assembler { recipe: String },
    Furnace,
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
            Function::Assembler { .. } | Function::Furnace => 3,
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
#[derive(Debug, Clone, Copy)]
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
    grid: FnvHashMap<Point, usize>,
}

impl Pcb {
    pub fn new() -> Pcb {
        Pcb {
            entities: Vec::new(),
            grid: FnvHashMap::default(),
        }
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
        self.place_entity_on_grid(entity, index);
    }

    fn entity_tiles<'a>(entity: &'a Entity) -> impl Iterator<Item=Point> + 'a {
        let tiles = (0..entity.size_x()).flat_map(move |x| (0..entity.size_y()).map(move |y| Point::new(x, y)));
        let tiles_origin = entity.location.coords;
        tiles.map(move |t| t + tiles_origin)
    }
    fn place_entity_on_grid(&mut self, entity: &Entity, index: usize) {
        for tile in Pcb::entity_tiles(entity) {
            let prev = self.grid.insert(tile, index);
            assert!(prev.is_none());
        }
    }
    pub fn entities<'a>(&'a self) -> impl Iterator<Item=&'a Entity> + Clone {
        self.entities.iter().filter_map(|o| o.as_ref())
    }

    pub fn remove_at(&mut self, point: (i32, i32)) {
        if let Some(i) = self.grid.remove(&Point::new(point.0, point.1)) {
            if let Some(e) = std::mem::replace(&mut self.entities[i], None) {
                for tile in Pcb::entity_tiles(&e) { self.grid.remove(&tile); }
            }
        }
    }
    pub fn entity_at(&self, point: Point) -> Option<&Entity> {
        self.grid.get(&point).and_then(|&i| self.entities[i].as_ref())
    }
    pub fn is_blocked(&self, point: Point) -> bool {
        self.entity_at(point).is_some()
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
