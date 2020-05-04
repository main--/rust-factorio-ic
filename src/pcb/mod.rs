use nalgebra::geometry::Point2;
use nalgebra::base::Vector2;
use std::borrow::Borrow;

pub type Point = Point2<i32>;
pub type Vector = Vector2<i32>;

mod hashmap;
mod naive;
mod grid;

pub use naive::NaivePcb;
pub use hashmap::HashmapPcb;
pub use grid::GridPcb;

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

    pub fn overlaps(&self, p: Point) -> bool {
        (self.location.x <= p.x)
            && (self.location.x + self.size_x() > p.x)
            && (self.location.y <= p.y)
            && (self.location.y + self.size_y() > p.y)
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


pub trait Pcb: Default + Clone where for<'a> Self: PcbRef<'a> {
    fn add(&mut self, entity: impl Borrow<Entity>);
    fn add_all<I>(&mut self, iter: I) where I: IntoIterator, I::Item: Borrow<Entity> {
        for e in iter { self.add(e); }
    }
    fn remove_at(&mut self, loc: Point);
    fn replace(&mut self, entity: impl Borrow<Entity>) {
        self.remove_at(entity.borrow().location);
        self.add(entity);
    }

    fn entity_at(&self, loc: Point) -> Option<&Entity>;
    fn is_blocked(&self, point: Point) -> bool {
        self.entity_at(point).is_some()
    }
}

pub trait PcbRef<'a> {
    type EntityIter: Iterator<Item=&'a Entity> + Clone;
    fn entities(&'a self) -> Self::EntityIter;
    fn entity_rect(&'a self) -> Rect {
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

fn entity_tiles<'a>(entity: &'a Entity) -> impl Iterator<Item=Point> + 'a {
    let tiles = (0..entity.size_x()).flat_map(move |x| (0..entity.size_y()).map(move |y| Point::new(x, y)));
    let tiles_origin = entity.location.coords;
    tiles.map(move |t| t + tiles_origin)
}

