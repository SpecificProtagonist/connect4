mod lib;

use std::{
    io::{stdin, BufRead},
    time::Instant,
    todo,
};
use structopt::{clap, StructOpt};

use lib::*;

/// Play Connect 4 against the computer or let the AI fight it out.
#[derive(StructOpt, Clone, Copy)]
#[structopt(name = "connect4")]
struct Opts {
    /// Game mode: PvP, PvC, CvC
    mode: GameMode,
    /// AI search tree depth.
    /// Computation time rises exponentially width depth.
    #[structopt(default_value = "8")]
    depth: u8,
    /// Optional seed for the AI
    seed: Option<u64>,
    /// Tell the AI to wait for the player to press enter
    #[structopt(long)]
    no_auto: bool,
    /// Print total game time
    #[structopt(long, conflicts_with("no-auto"))]
    time: bool, // TODO: Playing field size & winning_lenght
}

use structopt::clap::arg_enum;
arg_enum! {
#[derive(Clone, Copy)]
    enum GameMode {
        PvP,
        PvC,
        CvC,
    }
}

fn main() {
    let mut options = Opts::from_args();
    options.seed.get_or_insert_with(random_seed);
    match options.mode {
        GameMode::CvC => ai_vs_ai(options),
        GameMode::PvC => todo!(),
        GameMode::PvP => todo!(),
    }
}

fn random_seed() -> u64 {
    let mut buf = [0; 8];
    getrandom::getrandom(&mut buf).unwrap();
    u64::from_be_bytes(buf)
}

fn ai_vs_ai(config: Opts) {
    let mut rng = oorandom::Rand32::new(config.seed.unwrap());
    let mut pick = |possible: NextMove| {
        if possible.len() > 0 {
            Some(possible[rng.rand_u32() as usize % possible.len()])
        } else {
            None
        }
    };

    let time_start = Instant::now();

    let mut state = Default::default();
    loop {
        let (next_move, _) = find_next_move(&state, config.depth, true);

        if config.no_auto {
            let _ = stdin().lock().read_line(&mut String::new());
        }

        if let Some(column) = pick(next_move) {
            println!("Player {:?} plays column {}", state.player(), column,);
            match state.try_move(column) {
                MoveResult::State(next) => {
                    state = next;
                    println!("{}", state.print_board())
                }
                MoveResult::Victory => {
                    println!("Victory!");
                    break;
                }
                MoveResult::Impossible => unreachable!(),
            }
        } else {
            println!("Draw!");
            break;
        }
    }

    let time_end = Instant::now();
    if config.time {
        println!("Time: {}", (time_end - time_start).as_secs_f32());
    }
}
