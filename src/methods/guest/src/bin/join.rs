use fleetcore::{BaseInputs, BaseJournal};
use risc0_zkvm::guest::env;
use sha2::{Digest as _, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
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

// IMPORTANT:This code follows the rules of the classical Battleship game.
// Boats must be placed in a straight line (either horizontally or vertically), cannot touch each other either directly or diagonally, and must be of specific sizes.
// The definition of classical Battleship comes from the internet, and disagrees with my childhood memories.
// Not in the scope of this course, but important to note that the game has many variations, and this code implements one of them.
fn validate_fleet_placement(board: &[u8]) -> Result<(), String> {
    // Expected ship sizes: 2 submarines (size 1), 2 cruisers (size 2), 
    // 1 destroyer (size 3), 1 battleship (size 4), 1 carrier (size 5)
    let expected_ships = vec![1, 1, 2, 2, 3, 4, 5];
    let total_squares = expected_ships.iter().sum::<i32>(); // Should be 18

    // Check if board has the correct number of squares
    if board.len() != total_squares as usize {
        return Err(format!("Invalid number of ship squares: expected {}, got {}", 
                         total_squares, board.len()));
    }

    // Check for duplicate squares
    let unique_squares: HashSet<_> = board.iter().collect();
    if unique_squares.len() != board.len() {
        return Err("Duplicate squares found".to_string());
    }

    // Check if all squares are within the valid range (0-99)
    if board.iter().any(|&sq| sq > 99) {
        return Err("Invalid square coordinates".to_string());
    }

    // Use bitmask for faster lookups
    let mut grid = [false; 100];
    for &pos in board {
        grid[pos as usize] = true;
    }

    // Find all ships by looking for connected squares
    let mut visited = [false; 100];
    let mut ships = Vec::new();

    for &start in board {
        if visited[start as usize] {
            continue;
        }

        // BFS to find connected component
        let mut ship = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(start);
        visited[start as usize] = true;

        while let Some(current) = queue.pop_front() {
            ship.push(current);

            // Check adjacent squares (up, down, left, right only)
            let row = current / 10;
            let col = current % 10;
            
            let adjacent = [
                if row > 0 { Some(current - 10) } else { None },     // Up
                if row < 9 { Some(current + 10) } else { None },     // Down
                if col > 0 { Some(current - 1) } else { None },      // Left
                if col < 9 { Some(current + 1) } else { None },      // Right
            ];

            for adj in adjacent.iter().flatten() {
                if grid[*adj as usize] && !visited[*adj as usize] {
                    visited[*adj as usize] = true;
                    queue.push_back(*adj);
                }
            }
        }

        ships.push(ship);
    }

    // Validate ship counts
    let mut ship_counts = HashMap::new();
    for ship in &ships {
        *ship_counts.entry(ship.len()).or_insert(0) += 1;
    }

    let expected_counts = HashMap::from([(1, 2), (2, 2), (3, 1), (4, 1), (5, 1)]);
    if ship_counts != expected_counts {
        return Err(format!("Invalid ship configuration: expected {:?}, got {:?}", 
                         expected_counts, ship_counts));
    }

    // Validate ship shapes (must be straight lines)
    for ship in &ships {
        if ship.len() > 1 && !is_straight_line(ship) {
            return Err("Ships must be straight lines (no L-shapes allowed)".to_string());
        }
    }

    // Check that ships don't touch each other (including diagonally)
    if ships_touch_each_other(&ships) {
        return Err("Ships cannot touch each other either directly or diagonally".to_string());
    }

    Ok(())
}

fn is_straight_line(ship: &[u8]) -> bool {
    if ship.len() <= 1 {
        return true;
    }

    let positions: Vec<(u8, u8)> = ship.iter()
        .map(|&pos| (pos / 10, pos % 10))
        .collect();

    // Check if all positions are in the same row
    let same_row = positions.iter().all(|(row, _)| *row == positions[0].0);
    
    // Check if all positions are in the same column
    let same_col = positions.iter().all(|(_, col)| *col == positions[0].1);

    if !same_row && !same_col {
        return false;
    }

    // Check contiguity
    if same_row {
        let mut cols: Vec<u8> = positions.iter().map(|(_, col)| *col).collect();
        cols.sort_unstable();
        for i in 1..cols.len() {
            if cols[i] != cols[i-1] + 1 {
                return false;
            }
        }
    } else {
        let mut rows: Vec<u8> = positions.iter().map(|(row, _)| *row).collect();
        rows.sort_unstable();
        for i in 1..rows.len() {
            if rows[i] != rows[i-1] + 1 {
                return false;
            }
        }
    }

    true
}

fn ships_touch_each_other(ships: &[Vec<u8>]) -> bool {
    let occupied: HashSet<u8> = ships.iter()
        .flat_map(|ship| ship.iter())
        .copied()
        .collect();

    for ship in ships {
        for &pos in ship {
            let row = pos / 10;
            let col = pos % 10;

            // Check all 8 surrounding squares
            for dr in -1i32..=1 {
                for dc in -1i32..=1 {
                    if dr == 0 && dc == 0 {
                        continue; // Skip the current position
                    }

                    let new_row = row as i32 + dr;
                    let new_col = col as i32 + dc;

                    if new_row >= 0 && new_row < 10 && new_col >= 0 && new_col < 10 {
                        let adjacent_pos = (new_row as u8) * 10 + (new_col as u8);
                        
                        // If this adjacent position is occupied and not part of current ship
                        if occupied.contains(&adjacent_pos) && !ship.contains(&adjacent_pos) {
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}

fn main() {
    // read the input
    let mut _input: BaseInputs = env::read();
    let gameid = _input.gameid.clone();
    let fleet = _input.fleet.clone();
    let board = _input.board.clone();
    let random = _input.random.clone();
    
    // Validate the fleet placement 
    if board.len() < 18 {
        panic!("Not enough squares by boats");
    }
    // Now attempt the full validation
    match validate_fleet_placement(&board) {
        Ok(_) => {
            // Encrypt the fleet position by hashing the board with a nonce (random)
            let mut hasher = Sha256::new();
            hasher.update(&board);
            hasher.update(random.as_bytes());
            let sha2_digest_output = hasher.finalize();

            // Convert the SHA256 hash to a risc0_zkvm::Digest
            let committed_board_hash = risc0_zkvm::Digest::from(<[u8; 32]>::from(sha2_digest_output));

            // Generate the keys from the random string
            let (signing_key, verifying_key) = generate_keys_from_random(&random);

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
                verifying_key: Some(verifying_key.to_bytes().to_vec()),
            };

            // Successfully commit the output
            env::commit(&output);
        },
        Err(err) => panic!("VALIDATION ERROR: {}", err),
    }
}

