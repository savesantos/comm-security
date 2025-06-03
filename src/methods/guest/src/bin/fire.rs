use fleetcore::{FireInputs, FireJournal};
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

    // Generate the keys from the random string
    let (signing_key, _verifying_key) = generate_keys_from_random(&random);

    // Join the whole data into a single vector
    let mut data = Vec::new();
    data.extend_from_slice(&gameid.as_bytes());
    data.extend_from_slice(&fleet.as_bytes());
    data.extend_from_slice(&committed_board_hash.as_bytes());
    data.extend_from_slice(&target.as_bytes());
    data.extend_from_slice(&pos.to_le_bytes());

    // Sign the data
    let signature = signing_key.sign(&data);
    
    // create the output
    let output = FireJournal {
        gameid: gameid,
        fleet: fleet,
        board: committed_board_hash,
        target: target,
        pos: pos,
        signature: signature.to_vec(),
    };

    // write public output to the journal
    env::commit(&output);
}
