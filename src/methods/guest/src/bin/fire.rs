use fleetcore::{FireInputs, FireJournal};
use risc0_zkvm::guest::env;
use sha2::{Digest as _, Sha256};

fn main() {
    // read the input
    let _input: FireInputs = env::read();
    let gameid = _input.gameid.clone();
    let fleet = _input.fleet.clone();
    let board = _input.board.clone();
    let random = _input.random.clone();
    let target = _input.target.clone();
    let pos = _input.pos.clone();

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
        gameid: gameid,
        fleet: fleet,
        board: committed_board_hash,
        target: target,
        pos: pos,
    };

    // write public output to the journal
    env::commit(&output);
}
