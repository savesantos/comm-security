use fleetcore::{BaseInputs, BaseJournal};
use risc0_zkvm::guest::env;
use sha2::{Digest as _, Sha256};

fn main() {
    // read the input
    let input: BaseInputs = env::read();
    
    // Validate it's this player's turn to wave (same logic as fire)
    if input.game_next_player.as_ref() != Some(&input.fleet) {
        panic!("Not your turn to wave");
    }
    
    // Validate no one is waiting to report (same logic as fire)
    if input.game_next_report.is_some() {
        panic!("Cannot wave while someone needs to report");
    }
    
    let gameid = input.gameid.clone();
    let fleet = input.fleet.clone();
    let board = input.board.clone();
    let random = input.random.clone();

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
