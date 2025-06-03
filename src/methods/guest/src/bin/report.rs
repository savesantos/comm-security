use fleetcore::{FireInputs, ReportJournal};
use risc0_zkvm::guest::env;
use sha2::{Digest as _, Sha256};
use ed25519_dalek::{SigningKey, Signer};

fn generate_keys_from_random(random: &str) -> (SigningKey, ed25519_dalek::VerifyingKey) {
    // Create a deterministic seed from the random string
    let mut hasher = Sha256::new();
    hasher.update(random.as_bytes());
    let seed_hash = hasher.finalize();
    
    // Take first 32 bytes as seed for Ed25519
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&seed_hash[..32]);
    
    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();
    
    (signing_key, verifying_key)
}

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

    // Generate the keys from the random string
    let (signing_key, _verifying_key) = generate_keys_from_random(&random);

    // Join the whole data into a single vector
    let mut data = Vec::new();
    data.extend_from_slice(&gameid.as_bytes());
    data.extend_from_slice(&fleet.as_bytes());
    data.extend_from_slice(&committed_board_hash.as_bytes());
    data.extend_from_slice(&report.as_bytes());
    data.extend_from_slice(&pos.to_le_bytes());
    data.extend_from_slice(&committed_new_board_hash.as_bytes());

    // Sign the data
    let signature = signing_key.sign(&data);
    
    // Create the output journal with the validated report
    let output = ReportJournal {
        gameid,
        fleet,
        report,
        pos,
        board: committed_board_hash,
        next_board: committed_new_board_hash,
        signature: signature.to_vec(),
    };
    
    // write public output to the journal
    env::commit(&output);
}
