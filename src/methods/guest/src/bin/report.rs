use fleetcore::{FireInputs, ReportJournal};
use risc0_zkvm::guest::env;
use risc0_zkvm::Digest;
use sha2::{Digest as _, Sha256};

fn main() {
    // read the input
    let input: FireInputs = env::read();
    let gameid = input.gameid.clone();
    let fleet = input.fleet.clone();
    let board = input.board.clone();
    let random = input.random.clone();
    let report = input.target.clone();
    let pos = input.pos;
    
    // Parse the board from the input
    // The board is expected to be a Vec<u8> with the positions of ships
    // Validate that the report ("Hit" or "Miss") is accurate
    let board_vec = board.iter().map(|&b| b as u8).collect::<Vec<u8>>();
    
    // Check if the position is in the board (ship positions)
    let is_hit = board_vec.contains(&pos);
    
    // Validate that the report matches the actual state
    let is_valid_report = match report.as_str() {
        "Hit" => is_hit,
        "Miss" => !is_hit,
        _ => panic!("Report must be 'Hit' or 'Miss'"),
    };
    
    if !is_valid_report {
        panic!("Report does not match the actual board state");
    }
    
    // Create the SHA256 hash of the board
    let mut hasher = Sha256::new();
    hasher.update(&board);
    hasher.update(random.as_bytes());
    let sha2_digest_output = hasher.finalize();

    // Convert the SHA256 hash to a risc0_zkvm::Digest
    let committed_board_hash = risc0_zkvm::Digest::from(<[u8; 32]>::from(sha2_digest_output));

    // If player was hit, remove the position from the board and create a new board hash
    let mut new_board = board_vec.clone();
    if is_hit {
        // Remove the position from the board
        new_board.retain(|&x| x != pos);
    }

    // Create a new SHA256 hash for the updated board
    let mut new_hasher = Sha256::new();
    new_hasher.update(&new_board);
    new_hasher.update(random.as_bytes());
    let new_sha2_digest_output = new_hasher.finalize();

    // Convert the new SHA256 hash to a risc0_zkvm::Digest
    let committed_new_board_hash = risc0_zkvm::Digest::from(<[u8; 32]>::from(new_sha2_digest_output));
    
    // Create the output journal with the validated report
    let output = ReportJournal {
        gameid,
        fleet,
        report,
        pos,
        board: committed_board_hash,
        next_board: committed_new_board_hash,
    };
    
    // write public output to the journal
    env::commit(&output);
}
