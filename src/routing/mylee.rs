use std::iter;

use bumpalo::Bump;
use fehler::throws;
use either::Either;
use ndarray::{Array2, Array3};

use crate::pcb::{Direction, Pcb, Point, Vector, ALL_DIRECTIONS, Entity, Function, NeededWire, WireKind, Rect};
use crate::routing::{apply_lee_path, LogisticRoute, insert_underground_belts};

bitflags::bitflags! {
    pub struct Options: u64 {
        const PREFER_SAME_DIRECTION = 0b00000001;
        const USE_UNDERGROUND_BELTS = 0b00000010;
        const VISITED_WITH_DIRECTIONS = 0b00000100;
    }
}


#[throws(())]
pub fn mylee(pcb: &mut impl Pcb, &NeededWire { from, to, ref wire_kind }: &NeededWire, opts: Options) {
    let path = if opts.contains(Options::VISITED_WITH_DIRECTIONS) {
        mylee_internal::<_, WithDirections>(pcb, &ALL_DIRECTIONS, from, to, opts, wire_kind)
    } else {
        mylee_internal::<_, WithoutDirections>(pcb, &ALL_DIRECTIONS, from, to, opts, wire_kind)
    };

    apply_lee_path(pcb, from, path.ok_or(())?, wire_kind);
}

struct MazewalkerHistoryEntry<'a> {
    route: LogisticRoute,
    prev: MazewalkerHistory<'a>,
}
type MazewalkerHistory<'a> = Option<&'a MazewalkerHistoryEntry<'a>>;

struct Mazewalker<'a> {
    bump: &'a Bump,
    pos: Point,
    path: MazewalkerHistory<'a>,
}
impl<'a> Mazewalker<'a> {
    fn new(bump: &'a Bump, pos: Point) -> Self {
        Self {
            bump,
            pos,
            path: None,
        }
    }
    fn history_rev(&self) -> impl Iterator<Item=&'a LogisticRoute> + 'a {
        let mut path = self.path;
        iter::from_fn(move || {
            match path {
                Some(x) => {
                    let r = &x.route;
                    path = x.prev;
                    Some(r)
                }
                None => None,
            }
        })
    }
    fn conflicts_with_own_path(&self, test: Point) -> bool {
        // go backwards from current position
        let mut pos = self.pos;
        for &belt in self.history_rev() {
            match belt {
                LogisticRoute::Normal(dir) => {
                    pos -= dir.to_vector();
                    if pos == test {
                        return true;
                    }
                }
                LogisticRoute::Underground { dir, gap } => {
                    // underground end tile
                    pos -= dir.to_vector();
                    if pos == test {
                        return true;
                    }
                    pos -= dir.to_vector() * (gap + 1);
                    if pos == test {
                        return true;
                    }
                }
            }
        }
        false
    }
    fn append_step(&self, pos: Point, route: LogisticRoute) -> Self {
        Mazewalker {
            bump: self.bump,
            pos,
            path: Some(self.bump.alloc(MazewalkerHistoryEntry {
                route,
                prev: self.path,
            }))
        }
    }
    fn into_history_vec(self) -> Vec<LogisticRoute> {
        let mut v: Vec<_> = self.history_rev().copied().collect();
        v.reverse();
        v
    }
}

trait VisitedArray {
    fn new(x: i32, y: i32) -> Self;
    fn get(&self, x: i32, y: i32, dir: Direction) -> bool;
    fn set(&mut self, x: i32, y: i32, dir: Direction);
}
struct WithDirections(Array3<bool>);
struct WithoutDirections(Array2<bool>);

impl VisitedArray for WithDirections {
    fn new(x: i32, y: i32) -> Self {
        WithDirections(Array3::default((x as usize, y as usize, 4)))
    }

    fn get(&self, x: i32, y: i32, dir: Direction) -> bool {
        *self.0.get((x as usize, y as usize, dir as usize)).unwrap_or(&false)
    }

    fn set(&mut self, x: i32, y: i32, dir: Direction) {
        *self.0.get_mut((x as usize, y as usize, dir as usize)).unwrap() = true;
    }
}
impl VisitedArray for WithoutDirections {
    fn new(x: i32, y: i32) -> Self {
        WithoutDirections(Array2::default((x as usize, y as usize)))
    }

    fn get(&self, x: i32, y: i32, _: Direction) -> bool {
        *self.0.get((x as usize, y as usize)).unwrap_or(&false)
    }

    fn set(&mut self, x: i32, y: i32, _: Direction) {
        *self.0.get_mut((x as usize, y as usize)).unwrap() = true;
    }
}

struct Visited<G> {
    basis: Vector,
    grid: G,
}
impl<G: VisitedArray> Visited<G> {
    fn new(bounds: Rect) -> Self {
        let basis = bounds.a.coords;
        let size = bounds.b.coords - basis;
        Self {
            basis,
            grid: G::new(size.x, size.y),
        }
    }
    fn insert(&mut self, point: Point, dir: Direction) {
        let point = point - self.basis;

        self.grid.set(point.x, point.y, dir);
    }

    fn contains(&self, point: Point, dir: Direction) -> bool {
        let point = point - self.basis;
        self.grid.get(point.x, point.y, dir)
    }
}

fn mylee_internal<P: Pcb, G: VisitedArray>(
    pcb: &P, moveset: &[Direction], from: Point, to: Point, opts: Options, kind: &WireKind,
) -> Option<Vec<LogisticRoute>> {
    // ensure enough space around possible entities to possibly lay a belt around everything,
    // including a possible underground belt out, followed by an underground belt back in
    // and the connection loop
    let mut bounds = pcb.entity_rect();
    bounds.a += Vector::new(-2, -2);
    bounds.b += Vector::new(2, 2);

    let mut visited = Visited::<G>::new(bounds);

    let bump = Bump::new();

    // TODO: there's probably a much better algorithm based around some kind of cost heuristic
    let mut walkers = vec![Mazewalker::new(&bump, from)];
    let mut next_walkers = Vec::new();
    while !walkers.is_empty() {
        // println!("{} walkers {} visited", walkers.len(), visited.len());

        for walker in walkers.drain(..) {
            // println!("{} vs {}", walker.pos, to);

            let prev_step = walker.history_rev().next();
            let base_moveset = match prev_step {
                Some(LogisticRoute::Underground { dir, .. }) => Either::Left(ALL_DIRECTIONS.iter().filter(move |d| **d != dir.opposite_direction())),
                Some(LogisticRoute::Normal(_)) | None => Either::Right(moveset.iter()),
            };

            let prefer_direction =  if opts.contains(Options::PREFER_SAME_DIRECTION) {
                prev_step.map(LogisticRoute::direction)
            } else {
                None
            };

            for dir in prefer_direction.into_iter().chain(base_moveset.copied()) {
                let goto = walker.pos + dir.to_vector();
                if goto == to {
                    let mut path = walker.into_history_vec();
                    path.push(LogisticRoute::Normal(dir));
                    if !opts.contains(Options::USE_UNDERGROUND_BELTS) {
                        path = insert_underground_belts(path.into_iter().map(|b| match b {
                            LogisticRoute::Normal(d) => d,
                            _ => unreachable!(),
                        }), kind.gap_size());
                    }
                    return Some(path);
                }
                if pcb.is_blocked(goto) || visited.contains(goto, dir) || !bounds.contains(goto)
                    || (opts.contains(Options::VISITED_WITH_DIRECTIONS) && walker.conflicts_with_own_path(goto))
                {
                    continue;
                }

                // prevent accidental pipe connections
                if let WireKind::Pipe(kind) = kind {
                    let adjacents = [Vector::new(1, 0), Vector::new(-1, 0), Vector::new(0, 1), Vector::new(0, -1)];
                    let has_conflicting_pipes = adjacents.iter().map(|a| pcb.entity_at(goto + a)).any(|f| {
                        match f {
                            Some(Entity { function: Function::Pipe(t), .. }) => t != kind,
                            _ => false,
                        }
                    });
                    if has_conflicting_pipes {
                        continue;
                    }
                }

                visited.insert(goto, dir);

                // normal belt in that direction
                next_walkers.push(walker.append_step(goto, LogisticRoute::Normal(dir)));
            }

            // underground belts in the direction the last belt is pointing
            if opts.contains(Options::USE_UNDERGROUND_BELTS) {
                let dir = match prev_step {
                    Some(belt) => belt.direction(),
                    None => continue,
                };
                for gap in 0..=(kind.gap_size() as i32) {
                    let underground_end = walker.pos + (dir.to_vector() * (gap + 1));
                    // check for no interference with other underground belts in the way
                    match pcb.entity_at(underground_end) {
                        Some(Entity { function: Function::UndergroundBelt(intersecting_dir, _), .. })
                            if intersecting_dir.is_same_axis(dir) && kind == &WireKind::Belt => break,
                        Some(Entity { function: Function::UndergroundPipe(intersecting_dir), .. })
                            if intersecting_dir.is_same_axis(dir) && kind != &WireKind::Belt => break,
                        Some(_) => continue,
                        _ => (),
                    }
                    // we can't land directly on the field we want to reach with an underground belt
                    if underground_end == to || visited.contains(underground_end, dir) || !bounds.contains(underground_end)
                        || (opts.contains(Options::VISITED_WITH_DIRECTIONS) && walker.conflicts_with_own_path(underground_end))
                    {
                        continue;
                    }

                    let goto = underground_end + dir.to_vector();
                    if visited.contains(goto, dir) || !bounds.contains(goto) || pcb.is_blocked(goto)
                        || (opts.contains(Options::VISITED_WITH_DIRECTIONS) && walker.conflicts_with_own_path(goto))
                    {
                        continue;
                    }

                    visited.insert(underground_end, dir);
                    visited.insert(goto, dir);


                    let next = walker.append_step(goto, LogisticRoute::Underground { dir, gap });
                    if goto == to {
                        return Some(next.append_step(goto, LogisticRoute::Normal(dir)).into_history_vec());
                    }

                    next_walkers.push(next);
                }
            }
        }

        std::mem::swap(&mut walkers, &mut next_walkers);
    }
    None
}

