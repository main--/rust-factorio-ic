use std::iter;

use bumpalo::Bump;
use fehler::throws;
use either::Either;
use fnv::FnvHashSet;

use crate::pcb::{Direction, Pcb, Point, Vector, ALL_DIRECTIONS, Entity, Function, NeededWire, WireKind};
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
    let path = mylee_internal(pcb, &ALL_DIRECTIONS, from, to, opts, wire_kind).ok_or(())?;

    apply_lee_path(pcb, from, path, wire_kind);
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
        std::iter::from_fn(move || {
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

struct Visited {
    with_directions: bool,
    fields: FnvHashSet<Point>,
    fields_directions: FnvHashSet<(Point,Direction)>,
}

impl Visited {
    fn new(with_directions: bool) -> Visited {
        Visited {
            with_directions,
            fields: Default::default(),
            fields_directions: Default::default(),
        }
    }

    fn insert(&mut self, point: Point, dir: Direction) {
        if self.with_directions {
            self.fields_directions.insert((point, dir));
        } else {
            self.fields.insert(point);
        }
    }

    fn contains(&self, point: Point, dir: Direction) -> bool {
        if self.with_directions {
            self.fields_directions.contains(&(point, dir))
        } else {
            self.fields.contains(&point)
        }
    }

    fn len(&self) -> usize {
        if self.with_directions {
            self.fields_directions.len()
        } else {
            self.fields.len()
        }
    }
}

fn mylee_internal(
    pcb: &impl Pcb, moveset: &[Direction], from: Point, to: Point, opts: Options, kind: &WireKind,
) -> Option<Vec<LogisticRoute>> {
    // ensure enough space around possible entities to possibly lay a belt around everything,
    // including a possible underground belt out, followed by an underground belt back in
    // and the connection loop
    let mut bounds = pcb.entity_rect();
    bounds.a += Vector::new(-2, -2);
    bounds.b += Vector::new(2, 2);

    let mut visited = Visited::new(opts.contains(Options::VISITED_WITH_DIRECTIONS));

    let bump = Bump::new();

    // TODO: there's probably a much better algorithm based around some kind of cost heuristic
    let mut walkers = vec![Mazewalker::new(&bump, from)];
    while !walkers.is_empty() {
        // println!("{} walkers {} visited", walkers.len(), visited.len());

        for walker in std::mem::replace(&mut walkers, Vec::new()) {
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
                walkers.push(walker.append_step(goto, LogisticRoute::Normal(dir)));
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

                    walkers.push(next);
                }
            }
        }
    }
    None
}

