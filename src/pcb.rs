#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
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
    pub x: i32,
    pub y: i32,
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
        (self.x <= x)
            && (self.x + self.size_x() > x)
            && (self.y <= y)
            && (self.y + self.size_y() > y)
    }
}

pub type NeededWires = Vec<((i32, i32), (i32, i32))>;

#[derive(Debug, Clone)]
pub struct Pcb {
    entities: Vec<Entity>,
}

impl Pcb {
    pub fn new() -> Pcb {
        Pcb {
            entities: Vec::new(),
        }
    }
    pub fn entities(&self) -> &Vec<Entity> {
        &self.entities
    }
    pub fn entities_mut(&mut self) -> &mut Vec<Entity> {
        &mut self.entities
    }
}
