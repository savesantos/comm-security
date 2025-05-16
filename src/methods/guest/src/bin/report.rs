use fleetcore::{FireInputs, ReportJournal};
use risc0_zkvm::guest::env;
use risc0_zkvm::Digest;
use sha2::{Digest as Sha2Digest, Sha256};

fn main() {
    // Read the input
    let input: FireInputs = env::read();
    
    // Create a commitment to the current board state using SHA256
    let mut hasher = Sha256::new();
    
    // Add board data to the hash
    for &cell in input.board.iter() {
        hasher.update([cell]);
    }
    
    // Add the random seed to the hash for additional entropy
    hasher.update(input.random.as_bytes());
    
    // Finalize the hash
    let hash_result = hasher.finalize();
    
    // Convert to Digest format
    let mut digest = [0u8; 32];
    digest.copy_from_slice(hash_result.as_slice());
    let board_commitment = Digest::from(digest);
    
    // Verify the shot on the board
    // The `pos` field indicates the position that was hit
    let pos = input.pos as usize;
    
    // Determine if it was a hit or miss
    // In a real implementation, we would check input.board[pos] to see if a ship is there
    let is_hit = if pos < input.board.len() && input.board[pos] > 0 {
        "Hit"
    } else {
        "Miss"
    };
    
    // Create a new board state after the shot
    let mut next_board = input.board.clone();
    if is_hit == "Hit" && pos < next_board.len() {
        // Mark the ship as hit by changing its value
        next_board[pos] = 0;
    }
    
    // Create a commitment to the next board state
    let mut next_hasher = Sha256::new();
    for &cell in next_board.iter() {
        next_hasher.update([cell]);
    }
    next_hasher.update(input.random.as_bytes());
    let next_hash_result = next_hasher.finalize();
    
    // Convert to Digest format
    let mut next_digest = [0u8; 32];
    next_digest.copy_from_slice(next_hash_result.as_slice());
    let next_board_commitment = Digest::from(next_digest);
    
    // Create the output journal
    let output = ReportJournal {
        gameid: input.gameid,
        fleet: input.fleet,
        report: is_hit.to_string(),
        pos: input.pos,
        board: board_commitment,
        next_board: next_board_commitment
    };
    
    // Write public output to the journal
    env::commit(&output);
}
