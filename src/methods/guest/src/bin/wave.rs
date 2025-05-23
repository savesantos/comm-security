use fleetcore::{BaseInputs, BaseJournal};
use risc0_zkvm::guest::env;
use risc0_zkvm::Digest;
use sha2::{Digest as _, Sha256};

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

    // create the output
    let output = BaseJournal {
        gameid: gameid,
        fleet: fleet,
        board: committed_board_hash,
    };

    // write public output to the journal
    env::commit(&output);
}
