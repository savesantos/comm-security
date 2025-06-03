use fleetcore::{BaseInputs, BaseJournal};
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
    let _input: BaseInputs = env::read();
    let gameid = _input.gameid.clone();
    let fleet = _input.fleet.clone();
    let board = _input.board.clone();
    let random = _input.random.clone();

    // Encrypt the fleet position by hashing the board with a nonce (random)
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

    // Sign the data
    let signature = signing_key.sign(&data);

    // create the output
    let output = BaseJournal {
        gameid: gameid,
        fleet: fleet,
        board: committed_board_hash,
        signature: signature.to_vec(),
        verifying_key: None, // verifying key is only sent in the join phase
    };

    // write public output to the journal
    env::commit(&output);
}
