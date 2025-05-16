use fleetcore::{BaseInputs, BaseJournal};
use risc0_zkvm::guest::env;
use risc0_zkvm::Digest;
use sha2::{Digest as Sha2Digest, Sha256};

fn main() {
    // Read the input
    let input: BaseInputs = env::read();
    
    // Create a commitment to the board state using SHA256
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
    
    // Create the output journal with the board commitment
    let output = BaseJournal {
        gameid: input.gameid,
        fleet: input.fleet,
        board: board_commitment,
    };
    
    // Write public output to the journal
    env::commit(&output);
}