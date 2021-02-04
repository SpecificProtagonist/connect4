use std::{
    ops::{Index, IndexMut},
    time::Instant,
};

use rayon::iter::{IndexedParallelIterator, IntoParallelRefMutIterator, ParallelIterator};
use smallvec::{smallvec, SmallVec};

const COLUMNS: u8 = 7;
const ROWS: u8 = 6;
const WINNING_LENGTH: u8 = 4;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Player {
    O,
    X,
}

impl Player {
    fn other(self) -> Self {
        if let Player::O = self {
            Player::X
        } else {
            Player::O
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Cell {
    Empty,
    Set(Player),
}

impl Default for Cell {
    fn default() -> Self {
        Cell::Empty
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct State([[Cell; COLUMNS as usize]; ROWS as usize], Player);

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Current player: {:?}", self.1)?;
        for row in &self.0 {
            for cell in row {
                write!(
                    f,
                    "{}",
                    match cell {
                        Cell::Empty => ".",
                        Cell::Set(Player::O) => "o",
                        Cell::Set(Player::X) => "x",
                    }
                )?;
            }
            writeln!(f, "")?;
        }
        Ok(())
    }
}

/// Column (left to right), then Row (top to bottom)
impl Index<(u8, u8)> for State {
    type Output = Cell;

    fn index(&self, index: (u8, u8)) -> &Self::Output {
        &self.0[index.1 as usize][index.0 as usize]
    }
}
impl IndexMut<(u8, u8)> for State {
    fn index_mut(&mut self, index: (u8, u8)) -> &mut Self::Output {
        &mut self.0[index.1 as usize][index.0 as usize]
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum MoveResult {
    Impossible,
    Victory,
    State(State),
}

impl State {
    pub fn turn(&self) -> usize {
        self.0
            .iter()
            .flatten()
            .filter(|cell| matches!(cell, Cell::Set(_)))
            .count()
    }

    pub fn try_move(&self, column: u8) -> MoveResult {
        if let Cell::Empty = self[(column, 0)] {
            // Let gravity do its thing
            fn row(state: &State, column: u8) -> u8 {
                for row in 1..ROWS {
                    if let Cell::Set(_) = state[(column, row)] {
                        return row - 1;
                    }
                }
                ROWS - 1
            }
            let row = row(self, column);

            // Check for horizontal line
            let mut min_column = column;
            for column in (0..column).rev() {
                if self[(column, row)] == Cell::Set(self.1) {
                    min_column = column;
                } else {
                    break;
                }
            }
            let mut max_column = column;
            for column in (column + 1)..COLUMNS {
                if self[(column, row)] == Cell::Set(self.1) {
                    max_column = column;
                } else {
                    break;
                }
            }
            if max_column - min_column + 1 >= WINNING_LENGTH {
                return MoveResult::Victory;
            }

            // Check for vertical line
            let mut min_row = row;
            for row in (0..row).rev() {
                if self[(column, row)] == Cell::Set(self.1) {
                    min_row = row;
                } else {
                    break;
                }
            }
            let mut max_row = row;
            for row in (row + 1)..ROWS {
                if self[(column, row)] == Cell::Set(self.1) {
                    max_row = row;
                } else {
                    break;
                }
            }
            if max_row - min_row + 1 >= WINNING_LENGTH {
                return MoveResult::Victory;
            }

            // Check for bottom-left to top-right
            let mut min = column;
            for offset in 1..(ROWS - row).min(column + 1) {
                if self[(column - offset, row + offset)] == Cell::Set(self.1) {
                    min -= 1;
                } else {
                    break;
                }
            }
            let mut max = column;
            for offset in 1..(row + 1).min(COLUMNS - column) {
                if self[(column + offset, row - offset)] == Cell::Set(self.1) {
                    max += 1;
                } else {
                    break;
                }
            }
            if max - min + 1 >= WINNING_LENGTH {
                return MoveResult::Victory;
            }

            // Check for top-left to bottom-right
            let mut min = column;
            for offset in 1..(row.min(column) + 1) {
                if self[(column - offset, row - offset)] == Cell::Set(self.1) {
                    min -= 1;
                } else {
                    break;
                }
            }
            let mut max = column;
            for offset in 1..(ROWS - row).min(COLUMNS - column) {
                if self[(column + offset, row + offset)] == Cell::Set(self.1) {
                    max += 1;
                } else {
                    break;
                }
            }
            if max - min + 1 >= WINNING_LENGTH {
                return MoveResult::Victory;
            }

            // Not a winning move
            MoveResult::State({
                let mut new = State(self.0, self.1.other());
                new[(column, row)] = Cell::Set(self.1);
                new
            })
        } else {
            MoveResult::Impossible
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Eval {
    ImmediateVictory,
    AssuredVictory,
    AssuredLoss,
    Neutral,
}

type NextMove = SmallVec<[u8; COLUMNS as usize]>;

fn find_next_move(state: &State, depth: u8, parallelize: bool) -> (NextMove, Eval) {
    let mut move_states: SmallVec<[(u8, State); COLUMNS as usize]> = SmallVec::new();
    for column in 0..7 {
        match state.try_move(column) {
            MoveResult::Victory => return (smallvec![column], Eval::ImmediateVictory),
            MoveResult::Impossible => (),
            MoveResult::State(next) => move_states.push((column, next)),
        }
    }

    let mut moves_evals: SmallVec<[_; COLUMNS as usize]> =
        smallvec![(255, Eval::Neutral); move_states.len()];
    let eval = |((column, state), res): (&mut (u8, State), &mut (u8, Eval))| {
        let eval = if depth > 0 {
            find_next_move(state, depth - 1, false).1
        } else {
            Eval::Neutral
        };
        *res = (*column, eval);
    };
    if parallelize {
        move_states
            .par_iter_mut()
            .zip(moves_evals.par_iter_mut())
            .for_each(eval);
    } else {
        move_states
            .iter_mut()
            .zip(moves_evals.iter_mut())
            .for_each(eval);
    }

    let eval = if moves_evals
        .iter()
        .all(|(_, sit)| matches!(sit, Eval::ImmediateVictory | Eval::AssuredVictory))
    {
        Eval::AssuredLoss
    } else if moves_evals
        .iter()
        .any(|(_, sit)| matches!(sit, Eval::AssuredLoss))
    {
        moves_evals.retain(|(_, sit)| matches!(sit, Eval::AssuredLoss));
        Eval::AssuredVictory
    } else {
        // Todo: rate, prefer moves where opponent takes longer to win
        moves_evals
            .retain(|(_, sit)| !matches!(sit, Eval::ImmediateVictory | Eval::AssuredVictory));
        Eval::Neutral
    };
    let next_moves = moves_evals.iter().map(|(column, ..)| *column).collect();
    (next_moves, eval)
}

fn main() {
    let seed = {
        let mut buf = [0; 8];
        getrandom::getrandom(&mut buf).unwrap();
        u64::from_be_bytes(buf)
    };
    let seed = 1;
    let mut rng = oorandom::Rand32::new(seed);
    let mut pick = |possible: NextMove| {
        if possible.len() > 0 {
            Some(possible[rng.rand_u32() as usize % possible.len()])
        } else {
            None
        }
    };

    println!("Seed: {}", seed);
    let time_start = Instant::now();

    let mut state = State(Default::default(), Player::O);
    loop {
        let (next_move, eval) = find_next_move(&state, 8, true);
        if let Some(column) = pick(next_move) {
            println!(
                "Player {:?} plays column {} (eval: {:?})",
                state.1, column, eval
            );
            if let Eval::ImmediateVictory = eval {
                println!("Victory!");
                break;
            } else {
                state = match state.try_move(column) {
                    MoveResult::State(next) => next,
                    _ => unreachable!(),
                };
                println!("{:?}", state)
            }
        } else {
            println!("Draw!");
            break;
        }
    }

    let time_end = Instant::now();
    println!("Time: {}", (time_end - time_start).as_secs_f32());
}

#[rustfmt::skip]
#[test]
fn test_winning_moves() {
    use Player::*;
    use Cell::*;

    let state = State(
        [
            [Empty,  Empty,  Empty,  Empty,  Empty, Set(X), Empty],
            [Empty,  Empty,  Empty,  Empty,  Empty, Set(X), Empty],
            [Empty,  Empty,  Empty,  Empty,  Empty, Set(O), Empty],
            [Set(X), Set(O), Empty,  Empty,  Empty, Set(X), Empty],
            [Set(X), Set(O), Set(O), Empty,  Empty, Set(X), Empty],
            [Set(X), Set(O), Set(O), Set(O), Empty, Set(X), Empty],
        ],
        Player::O
    );

    assert!(matches!(state.try_move(0), MoveResult::Victory));
    assert!(matches!(state.try_move(1), MoveResult::Victory));
    assert!(matches!(state.try_move(2), MoveResult::State(_)));
    assert!(matches!(state.try_move(3), MoveResult::State(_)));
    assert!(matches!(state.try_move(4), MoveResult::Victory));
    assert!(matches!(state.try_move(5), MoveResult::Impossible));
    assert!(matches!(state.try_move(6), MoveResult::State(_)));

    let state = State(
        [
            [Empty,  Empty,  Empty,  Empty,  Empty,  Empty,  Empty],
            [Empty,  Empty,  Empty,  Empty,  Set(O), Empty,  Empty],
            [Empty,  Empty,  Empty,  Empty,  Set(O), Set(O), Empty],
            [Empty,  Empty,  Set(X), Empty,  Set(O), Set(X), Set(X)],
            [Empty,  Empty,  Set(O), Empty,  Set(X), Set(X), Set(O)],
            [Set(X), Empty,  Set(O), Set(O), Set(X), Set(O), Set(X)],
        ],
        Player::O
    );
    assert!(matches!(state.try_move(0), MoveResult::State(_)));
    assert!(matches!(state.try_move(1), MoveResult::State(_)));
    assert!(matches!(state.try_move(2), MoveResult::State(_)));
    assert!(matches!(state.try_move(3), MoveResult::Victory));
    assert!(matches!(state.try_move(4), MoveResult::Victory));
    assert!(matches!(state.try_move(5), MoveResult::State(_)));
    assert!(matches!(state.try_move(6), MoveResult::State(_)));
}
