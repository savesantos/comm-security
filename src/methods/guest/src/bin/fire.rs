use fleetcore::{FireInputs, FireJournal};
use risc0_zkvm::guest::env;
use sha2::{Digest as _, Sha256};

fn main() {
    let input: FireInputs = env::read();
    
    // Validate it's this player's turn to fire
    if input.game_next_player.as_ref() != Some(&input.fleet) {
        panic!("Not your turn to fire");
    }
    
    // Validate no one is waiting to report
    if input.game_next_report.is_some() {
        panic!("Cannot fire while someone needs to report");
    }

    let fleet = input.fleet.clone();
    let board = input.board.clone();
    let random = input.random.clone();
    let target = input.target.clone();
    let pos = input.pos.clone();

    // Validate that target is not himself
    if fleet == target {
        panic!("Cannot fire at yourself");
    }

    // Validate that the position is within the board
    if pos > 99 {
        panic!("Position out of bounds");
    }

    // Validate that your fleet is not already sunk
    if board.len() < 1 {
        panic!("Your fleet is already sunk");
    }

    // Create the SHA256 hash of the board
    let mut hasher = Sha256::new();
    hasher.update(&board);
    hasher.update(random.as_bytes());
    let sha2_digest_output = hasher.finalize();

    // Convert the SHA256 hash to a risc0_zkvm::Digest
    let committed_board_hash = risc0_zkvm::Digest::from(<[u8; 32]>::from(sha2_digest_output));
    
    // create the output
    let output = FireJournal {
        gameid: input.gameid,
        fleet: input.fleet,
        board: committed_board_hash,
        target: input.target,
        pos: input.pos,
    };

    // write public output to the journal
    env::commit(&output);
}
