use fleetcore::{BaseInputs, BaseJournal};
use risc0_zkvm::guest::env;
use risc0_zkvm::Digest;
use sha2::{Digest as _, Sha256};
use std::collections::HashSet;

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

    // Find all ships by looking for connected squares
    let mut remaining_squares: HashSet<u8> = board.iter().copied().collect();
    let mut ships = Vec::new();

    while !remaining_squares.is_empty() {
        // Start with any remaining square
        let start = *remaining_squares.iter().next().unwrap();
        let mut ship = vec![start];
        remaining_squares.remove(&start);
        
        // Keep track of squares to check
        let mut to_check = vec![start];
        
        while !to_check.is_empty() {
            let current = to_check.pop().unwrap();
            
            // Check adjacent squares (up, down, left, right)
            let possible_adjacent = [
                // Up (if not on top row)
                if current >= 10 { Some(current - 10) } else { None },
                // Down (if not on bottom row)
                if current < 90 { Some(current + 10) } else { None },
                // Left (if not on leftmost column)
                if current % 10 != 0 { Some(current - 1) } else { None },
                // Right (if not on rightmost column)
                if current % 10 != 9 { Some(current + 1) } else { None },
            ];
            
            for adj in possible_adjacent.iter().flatten() {
                if remaining_squares.contains(adj) {
                    ship.push(*adj);
                    remaining_squares.remove(adj);
                    to_check.push(*adj);
                }
            }
        }
        
        // Add ship to the list
        ships.push(ship);
    }

    // Check if we have the right ships
    let mut ship_sizes: Vec<_> = ships.iter().map(|ship| ship.len() as i32).collect();
    ship_sizes.sort_unstable();
    
    if ship_sizes != expected_ships {
        return Err(format!("Invalid ship configuration: expected {:?}, got {:?}", 
                         expected_ships, ship_sizes));
    }

    // Validate ship shapes (must be straight lines)
    for ship in &ships {
        if ship.len() > 1 {
            let is_horizontal = ship.iter().all(|&sq| sq / 10 == ship[0] / 10);
            let is_vertical = ship.iter().all(|&sq| sq % 10 == ship[0] % 10);
            
            if !is_horizontal && !is_vertical {
                return Err("Ships must be straight lines".to_string());
            }
            
            // Check if the ship is contiguous
            if is_horizontal {
                let mut ship_coords: Vec<_> = ship.iter().map(|&sq| sq % 10).collect();
                ship_coords.sort_unstable();
                
                for i in 1..ship_coords.len() {
                    if ship_coords[i] != ship_coords[i-1] + 1 {
                        return Err("Ship has gaps".to_string());
                    }
                }
            } else { // is_vertical
                let mut ship_coords: Vec<_> = ship.iter().map(|&sq| sq / 10).collect();
                ship_coords.sort_unstable();
                
                for i in 1..ship_coords.len() {
                    if ship_coords[i] != ship_coords[i-1] + 1 {
                        return Err("Ship has gaps".to_string());
                    }
                }
            }
        }
    }

    Ok(())
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

            // create the output
            let output = BaseJournal {
                gameid: gameid,
                fleet: fleet,
                board: committed_board_hash,
            };

            // Successfully commit the output
            env::commit(&output);
        },
        Err(err) => panic!("VALIDATION ERROR: {}", err),
    }
}

