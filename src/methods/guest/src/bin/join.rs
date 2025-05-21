use fleetcore::{BaseInputs, BaseJournal};
use risc0_zkvm::guest::env; // Make sure env is imported for logging
use risc0_zkvm::Digest;
use sha2::{Digest as _, Sha256};
use std::collections::HashMap;

// Function to validate fleet placement
fn validate_fleet_placement(board_indices: &Vec<u8>) {
    env::log(&format!(
        "Validating fleet. Received cell count: {}, Expected: {}",
        board_indices.len(),
        EXPECTED_TOTAL_CELLS
    )); // Added log
    const EXPECTED_TOTAL_CELLS: usize = 18;
    let expected_ship_counts: HashMap<usize, usize> = [
        (1, 2), // 2 Submarines
        (2, 2), // 2 Cruisers
        (3, 1), // 1 Destroyer
        (4, 1), // 1 Battleship
        (5, 1), // 1 Carrier
    ]
    .iter()
    .cloned()
    .collect();

    // 1. Check total number of cells
    if board_indices.len() != EXPECTED_TOTAL_CELLS {
        panic!(
            "Invalid fleet: expected {} occupied cells, found {}",
            EXPECTED_TOTAL_CELLS,
            board_indices.len()
        );
    }

    let mut sorted_board = board_indices.clone();
    sorted_board.sort_unstable();

    // 2. Check for duplicate or out-of-bounds cell indices
    if sorted_board.is_empty() && EXPECTED_TOTAL_CELLS > 0 { // Should be caught by len check if EXPECTED_TOTAL_CELLS > 0
        panic!("Invalid fleet: board is empty.");
    }
    if !sorted_board.is_empty() {
        if sorted_board.last().map_or(false, |&idx| idx >= 100) {
             panic!("Invalid fleet: cell index out of bounds (must be 0-99).");
        }
        for i in 0..sorted_board.len() - 1 {
            if sorted_board[i] == sorted_board[i+1] {
                panic!("Invalid fleet: duplicate cell index {} found.", sorted_board[i]);
            }
            if sorted_board[i] >= 100 { // Should be caught by last element check too
                 panic!("Invalid fleet: cell index {} is out of bounds (0-99).", sorted_board[i]);
            }
        }
    }


    let mut visited_mask = [false; 100];
    let mut found_ships_counts: HashMap<usize, usize> = HashMap::new();

    for &cell_idx_u8 in &sorted_board {
        let cell_idx = cell_idx_u8 as usize;
        if visited_mask[cell_idx] {
            continue;
        }

        let r = cell_idx / 10;
        let c = cell_idx % 10;

        // Determine max possible horizontal length from (r,c)
        let mut horizontal_len = 1;
        for l in 1..5 { // Max ship length 5, so check 4 more cells
            let next_c = c + l;
            if next_c >= 10 { break; }
            if sorted_board.binary_search(&((r * 10 + next_c) as u8)).is_ok() {
                horizontal_len += 1;
            } else {
                break;
            }
        }

        // Determine max possible vertical length from (r,c)
        let mut vertical_len = 1;
        for l in 1..5 { // Max ship length 5, so check 4 more cells
            let next_r = r + l;
            if next_r >= 10 { break; }
            if sorted_board.binary_search(&(((next_r) * 10 + c) as u8)).is_ok() {
                vertical_len += 1;
            } else {
                break;
            }
        }
        
        let actual_ship_len;
        let is_horizontal;

        // 3. Check for L-shapes (ships must be straight lines)
        if horizontal_len > 1 && vertical_len > 1 {
            panic!("Invalid ship shape at ({},{}): ships must be straight lines.", r, c);
        } else if horizontal_len >= vertical_len { // Prioritize horizontal if equal or greater
            actual_ship_len = horizontal_len;
            is_horizontal = true;
        } else { // vertical_len > horizontal_len
            actual_ship_len = vertical_len;
            is_horizontal = false;
        }
        
        if actual_ship_len == 0 || actual_ship_len > 5 {
             panic!("Invalid ship length {} (max 5) for ship starting at ({},{}).", actual_ship_len, r, c);
        }

        // 4. Mark cells of this ship as visited and check for overlaps
        for i in 0..actual_ship_len {
            let current_ship_part_idx = if is_horizontal {
                r * 10 + (c + i)
            } else {
                (r + i) * 10 + c
            };
            
            // This check ensures the cell is part of the original board input.
            // (already implicitly handled by how horizontal_len/vertical_len are calculated)
            if sorted_board.binary_search(&(current_ship_part_idx as u8)).is_err() {
                 panic!("Logic error: ship part ({},{}) not in original board.", current_ship_part_idx/10, current_ship_part_idx%10);
            }

            if visited_mask[current_ship_part_idx] {
                 // This cell was already part of another ship, meaning an overlap.
                 // Since cell_idx itself was unvisited, this implies i > 0.
                 panic!("Overlapping ships detected at cell ({},{}).", current_ship_part_idx/10, current_ship_part_idx%10);
            }
            visited_mask[current_ship_part_idx] = true;
        }
        *found_ships_counts.entry(actual_ship_len).or_insert(0) += 1;
    }
    
    // Sanity check: all cells from input board must be visited
    for &cell_idx_u8 in &sorted_board {
        if !visited_mask[cell_idx_u8 as usize] {
            panic!("Cell {} ({},{}) was in input but not part of any identified ship.", cell_idx_u8, cell_idx_u8/10, cell_idx_u8%10);
        }
    }
    
    // 5. Compare found ship counts with expected counts
    let mut all_counts_match = true;
    let mut error_details = String::new();

    for (len, expected_count) in &expected_ship_counts {
        match found_ships_counts.get(len) {
            Some(found_count) => {
                if found_count != expected_count {
                    all_counts_match = false;
                    let detail = format!("Length {}: expected {}, found {}. ", len, expected_count, found_count);
                    error_details.push_str(&detail);
                    env::log(&detail); // Log mismatch
                }
            }
            None => {
                if *expected_count > 0 {
                    all_counts_match = false;
                    let detail = format!("Length {}: expected {}, found 0. ", len, expected_count);
                    error_details.push_str(&detail);
                    env::log(&detail); // Log missing ships
                }
            }
        }
    }

    for (len, found_count) in &found_ships_counts {
        if !expected_ship_counts.contains_key(len) && *found_count > 0 {
            all_counts_match = false;
            let detail = format!("Unexpected ships of length {}: found {}. ", len, found_count);
            error_details.push_str(&detail);
            env::log(&detail); // Log unexpected ships
        }
    }

    if !all_counts_match {
        env::log(&format!("Incorrect fleet composition: {}", error_details)); // Log before panic
        panic!("Incorrect fleet composition: {}", error_details);
    }
}

fn main() {

    // read the input
    let _input: BaseInputs = env::read();
    // TODO: do something with the input
    let gameid = _input.gameid.clone();
    let fleet = _input.fleet.clone();
    let board = _input.board.clone(); // This is Vec<u8>
    let random = _input.random.clone();

    // Validate the fleet placement
    validate_fleet_placement(&board); // Call the validation function

    // Encrypt the fleet position by hashing the board with a nonce (random)
    let mut hasher = Sha256::new();
    hasher.update(&board); // Hash the original board indices for commitment
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

    env::commit(&output);
}

