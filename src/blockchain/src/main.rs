// Remove the following 3 lines to enable compiler checkings
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]

use axum::{
    extract::Extension,
    response::{sse::Event, Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::StreamExt;
use rand::{seq::IteratorRandom, SeedableRng};
use risc0_zkvm::Digest;
use std::{
    collections::HashMap,
    error::Error,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use ed25519_dalek::{VerifyingKey, Verifier, Signature};

use fleetcore::{BaseJournal, Command, FireJournal, CommunicationData, ReportJournal};
use methods::{FIRE_ID, JOIN_ID, REPORT_ID, WAVE_ID, WIN_ID};

struct Player {
    name: String,
    current_state: Digest,
    last_turn_timestamp: u64,
    has_claimed_victory: bool,
    verifying_key: VerifyingKey,
}
struct Game {
    pmap: HashMap<String, Player>,
    next_player: Option<String>,
    next_report: Option<String>,
    first_victory_claim: Option<(String, u64)>, // (player_name, timestamp)
    victory_timeout_seconds: u64,
    first_shot_fired: bool,
}

#[derive(Clone)]
struct SharedData {
    tx: broadcast::Sender<String>,
    gmap: Arc<Mutex<HashMap<String, Game>>>,
    rng: Arc<Mutex<rand::rngs::StdRng>>,
}

#[tokio::main]
async fn main() {
    // Create a broadcast channel for log messages
    let (tx, _rx) = broadcast::channel::<String>(100);
    let shared = SharedData {
        tx: tx,
        gmap: Arc::new(Mutex::new(HashMap::new())),
        rng: Arc::new(Mutex::new(rand::rngs::StdRng::from_entropy())),
    };

    // Clone shared data for the timeout checker before moving it to the extension
    let timeout_checker = shared.clone();

    // Build our application with a route
    let app = Router::new()
        .route("/", get(index))
        .route("/logs", get(logs))
        .route("/chain", post(smart_contract))
        .layer(Extension(shared));

    // Run our app with hyper
    //let addr = SocketAddr::from(([127, 0, 0, 1], 3001));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));
    println!("Listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    
    // Start the timeout checker task
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            check_victory_timeouts(&timeout_checker).await;
        }
    });

    axum::serve(listener, app).await.unwrap();
}

// Handler to serve the HTML page
async fn index() -> Html<&'static str> {
    Html(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Blockchain Emulator</title>
        </head>
        <body>
            <h1>Registered Transactions</h1>          
            <ul id="logs"></ul>
            <script>
                const eventSource = new EventSource('/logs');
                eventSource.onmessage = function(event) {
                    const logs = document.getElementById('logs');
                    const log = document.createElement('li');
                    log.textContent = event.data;
                    logs.appendChild(log);
                };
            </script>
        </body>
        </html>
        "#,
    )
}

// Handler to manage SSE connections
#[axum::debug_handler]
async fn logs(Extension(shared): Extension<SharedData>) -> impl IntoResponse {
    let rx = BroadcastStream::new(shared.tx.subscribe());
    let stream = rx.filter_map(|result| async move {
        match result {
            Ok(msg) => Some(Ok(Event::default().data(msg))),
            Err(_) => Some(Err(Box::<dyn Error + Send + Sync>::from("Error"))),
        }
    });

    axum::response::sse::Sse::new(stream)
}

fn xy_pos(pos: u8) -> String {
    let x = pos % 10;
    let y = pos / 10;
    format!("{}{}", (x + 65) as char, y)
}

async fn smart_contract(
    Extension(shared): Extension<SharedData>,
    Json(input_data): Json<CommunicationData>,
) -> String {
    match input_data.cmd {
        Command::Join => handle_join(&shared, &input_data),
        Command::Fire => handle_fire(&shared, &input_data),
        Command::Report => handle_report(&shared, &input_data),
        Command::Wave => handle_wave(&shared, &input_data),
        Command::Win => handle_win(&shared, &input_data),
    }
}

fn handle_join(shared: &SharedData, input_data: &CommunicationData) -> String {
    // Verify the receipt first
    if input_data.receipt.verify(JOIN_ID).is_err() {
        shared.tx.send("Attempting to join game with invalid receipt".to_string()).unwrap();
        return "Could not verify receipt".to_string();
    }
    
    // Decode the journal
    let data: BaseJournal = input_data.receipt.journal.decode().unwrap();

    // Get verifying key from the communication data
    let verifying_key_bytes = match input_data.public_key.as_ref() {
        Some(pk) => pk,
        None => {
            shared.tx.send("Verifying key is missing in join request".to_string()).unwrap();
            return "Missing verifying key".to_string();
        }
    };

    // Convert bytes to VerifyingKey
    let verifying_key = match VerifyingKey::from_bytes(verifying_key_bytes.as_slice().try_into().unwrap()) {
        Ok(key) => key,
        Err(_) => {
            shared.tx.send("Invalid verifying key in join request".to_string()).unwrap();
            return "Invalid verifying key".to_string();
        }
    };

    // Convert signature bytes to Signature
    let signature = Signature::from_bytes(input_data.signature.as_slice().try_into().unwrap());

    // Verify the signature against the receipt data
    if verifying_key.verify(&input_data.receipt.journal.bytes.as_slice(), &signature).is_err() {
        shared.tx.send("Invalid signature in join request".to_string()).unwrap();
        return "Invalid signature".to_string();
    }

    let mut gmap = shared.gmap.lock().unwrap();
    
    // Get current timestamp for initializing player
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    // Check if game exists and if the first shot has been fired
    if let Some(existing_game) = gmap.get(&data.gameid) {
        // Check if the first shot has been fired
        if existing_game.first_shot_fired {
            shared.tx.send(format!("Cannot join game {} - game has already started (first shot fired)", data.gameid)).unwrap();
            return "Cannot join - game has already started".to_string();
        }
        
        // Check if player is already in the game
        if existing_game.pmap.contains_key(&data.fleet) {
            shared.tx.send(format!("Player {} already in game {}", data.fleet, data.gameid)).unwrap();
            return "Player already in game".to_string();
        }
    }
    
    // Create or get the game entry
    let game = gmap.entry(data.gameid.clone()).or_insert(Game {
        pmap: HashMap::new(),
        next_player: Some(data.fleet.clone()),
        next_report: None,
        first_victory_claim: None,
        victory_timeout_seconds: 30,
        first_shot_fired: false,
    });
    
    // Insert the player into the game
    let player_inserted = game.pmap.entry(data.fleet.clone()).or_insert_with(|| Player {
        name: data.fleet.clone(),
        current_state: data.board.clone(),
        last_turn_timestamp: current_time,
        has_claimed_victory: false,
        verifying_key: verifying_key,
    }).name == data.fleet;
    
    let mesg = if player_inserted {
        format!("{} joined game {}", data.fleet, data.gameid)
    } else {
        format!("Player already in game {}", data.gameid)
    };
    shared.tx.send(mesg).unwrap();
    "OK".to_string()
}

fn handle_fire(shared: &SharedData, input_data: &CommunicationData) -> String {
    // Verify the receipt first
    if input_data.receipt.verify(FIRE_ID).is_err() {
        shared.tx.send("Attempting to fire with invalid receipt".to_string()).unwrap();
        return "Could not verify receipt".to_string();
    }

    // Decode the journal
    let data: FireJournal = input_data.receipt.journal.decode().unwrap();
    let mut gmap = shared.gmap.lock().unwrap();

    // Check if the game exists
    let game = match gmap.get_mut(&data.gameid) {
        Some(game) => game,
        None => {
            shared.tx.send(format!("Game {} not found", data.gameid)).unwrap();
            return "Game not found".to_string();
        }
    };

    // Check if the target is in the game
    if !game.pmap.contains_key(&data.target) {
        shared.tx.send(format!("Target {} not found in game {}", data.target, data.gameid)).unwrap();
        return "Target not found".to_string();
    }

    // Check if the target is not the player itself
    if data.fleet == data.target {
        shared.tx.send(format!("Cannot fire at yourself in game {}", data.gameid)).unwrap();
        return "Cannot fire at yourself".to_string();
    }

    // Check if the player is in the game
    let player = match game.pmap.get_mut(&data.fleet) {
        Some(player) => player,
        None => {
            shared.tx.send(format!("Player {} not found in game {}", data.fleet, data.gameid)).unwrap();
            return "Player not found".to_string();
        }
    };

    // Get verifying key from player
    let verifying_key = &player.verifying_key;

    // Convert signature bytes to Signature
    let signature = Signature::from_bytes(input_data.signature.as_slice().try_into().unwrap());

    // Verify the signature against the receipt data
    if verifying_key.verify(&input_data.receipt.journal.bytes.as_slice(), &signature).is_err() {
        shared.tx.send("Invalid signature in fire request".to_string()).unwrap();
        return "Invalid signature".to_string();
    }

    // Check if someone has claimed victory and timeout is active
    if let Some((claimant, claim_time)) = &game.first_victory_claim {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if current_time - claim_time < game.victory_timeout_seconds {
            let remaining_time = game.victory_timeout_seconds - (current_time - claim_time);
            shared.tx.send(format!("Cannot fire during victory claim period. {} claimed victory. {} seconds remaining to contest by clicking on 'Win' button.", claimant, remaining_time)).unwrap();
            return "Cannot fire during victory claim period".to_string();
        }
    }

    // Check if player's board hash matches the current state (current saved board hash)
    if player.current_state != data.board {
        shared.tx.send(format!("Player {}'s board hash does not match the current state in game {}", data.fleet, data.gameid)).unwrap();
        return "Board hash mismatch".to_string();
    }

    // Check if it's the player's turn
    if game.next_player.as_ref() != Some(&data.fleet) {
        shared.tx.send(format!("Not {}'s turn in game {}", data.fleet, data.gameid)).unwrap();
        return "Not your turn".to_string();
    }

    // Check if someone has yet to report, including the player
    if game.next_report.is_some() {
        shared.tx.send(format!("Cannot fire until player {} has reported in game {}", game.next_report.as_ref().unwrap(), data.gameid)).unwrap();
        return format!("Cannot fire until player {} has reported", game.next_report.as_ref().unwrap()).to_string();
    }

    // Check if the target position is valid
    if data.pos > 99 {
        shared.tx.send(format!("Invalid target position {} in game {}", xy_pos(data.pos), data.gameid)).unwrap();
        return "Invalid target position".to_string();
    }

    // Get current timestamp
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    // Update the timestamp for the player who just reported
    player.last_turn_timestamp = current_time;

    // Mark that the first shot has been fired
    game.first_shot_fired = true;

    // Update who needs to report to the player that was just fired at
    game.next_report = Some(data.target.clone());
    
    // Update the next player (next_player will be attributed to the player that was just fired at after they report)
    game.next_player = None;
    
    // Send a message about the successful shot
    let msg = format!(
        "{} fired at {} in game {} at position {}",
        data.fleet,
        data.target,
        data.gameid,
        xy_pos(data.pos)
    );
    shared.tx.send(msg).unwrap();
    
    "OK".to_string()
}

fn handle_report(shared: &SharedData, input_data: &CommunicationData) -> String {
    // Verify the receipt first
    if input_data.receipt.verify(REPORT_ID).is_err() {
        shared.tx.send("Attempting to report with invalid receipt".to_string()).unwrap();
        return "Could not verify receipt".to_string();
    }

    // Decode the journal
    let data: ReportJournal = input_data.receipt.journal.decode().unwrap();
    let mut gmap = shared.gmap.lock().unwrap();

    // Check if the game exists
    let game = match gmap.get_mut(&data.gameid) {
        Some(game) => game,
        None => {
            shared.tx.send(format!("Game {} not found", data.gameid)).unwrap();
            return "Game not found".to_string();
        }
    };

    // Check if the player is in the game
    let player = match game.pmap.get_mut(&data.fleet) {
        Some(player) => player,
        None => {
            shared.tx.send(format!("Player {} not found in game {}", data.fleet, data.gameid)).unwrap();
            return "Player not found".to_string();
        }
    };

    // Get verifying key from player
    let verifying_key = &player.verifying_key;

    // Convert signature bytes to Signature
    let signature = Signature::from_bytes(input_data.signature.as_slice().try_into().unwrap());

    // Verify the signature against the receipt data
    if verifying_key.verify(&input_data.receipt.journal.bytes.as_slice(), &signature).is_err() {
        shared.tx.send("Invalid signature in report request".to_string()).unwrap();
        return "Invalid signature".to_string();
    }

    // Check if someone has claimed victory and timeout is active
    if let Some((claimant, claim_time)) = &game.first_victory_claim {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if current_time - claim_time < game.victory_timeout_seconds {
            let remaining_time = game.victory_timeout_seconds - (current_time - claim_time);
            shared.tx.send(format!("Cannot report during victory claim period. {} claimed victory. {} seconds remaining to contest by clicking on 'Win' button.", claimant, remaining_time)).unwrap();
            return "Cannot report during victory claim period".to_string();
        }
    }

    // Check if it's the player's turn to report
    if game.next_report.as_ref() != Some(&data.fleet) {
        shared.tx.send(format!("Not {}'s turn to report in game {}", data.fleet, data.gameid)).unwrap();
        return "Not your turn to report".to_string();
    }

    // Check if player's board hash matches the current state (current saved board hash)
    if player.current_state != data.board {
        shared.tx.send(format!("Player {}'s board hash does not match the current state in game {}", data.fleet, data.gameid)).unwrap();
        return "Board hash mismatch".to_string();
    }

    // Check if position is valid
    if data.pos > 99 {
        shared.tx.send(format!("Invalid position {} in game {}", xy_pos(data.pos), data.gameid)).unwrap();
        return "Invalid position".to_string();
    }

    // Check if the report is valid ("Hit" or "Miss")
    if data.report != "Hit" && data.report != "Miss" {
        shared.tx.send(format!("Invalid report {} in game {}", data.report, data.gameid)).unwrap();
        return "Invalid report".to_string();
    }

    // Update the player's board state
    if data.report == "Hit" {
        // Remove the position from the player's board
        player.current_state = data.next_board.clone();
    } else {
        // Update the player's board state to the next board
        player.current_state = data.next_board.clone();
    }

    // Update the next player to the player that was just reported
    game.next_player = Some(data.fleet.clone());
    game.next_report = None;

    // Send a message about the successful report
    let msg = format!(
        "{} reported {} at position {} in game {}",
        data.fleet,
        data.report,
        xy_pos(data.pos),
        data.gameid
    );
    shared.tx.send(msg).unwrap();

    "OK".to_string()
}

fn handle_wave(shared: &SharedData, input_data: &CommunicationData) -> String {
    // Verify the receipt first
    if input_data.receipt.verify(WAVE_ID).is_err() {
        shared.tx.send("Attempting to wave with invalid receipt".to_string()).unwrap();
        return "Could not verify receipt".to_string();
    }

    // Decode the journal
    let data: BaseJournal = input_data.receipt.journal.decode().unwrap();
    let mut gmap = shared.gmap.lock().unwrap();

    // Check if the game exists
    let game = match gmap.get_mut(&data.gameid) {
        Some(game) => game,
        None => {
            shared.tx.send(format!("Game {} not found", data.gameid)).unwrap();
            return "Game not found".to_string();
        }
    };

    // Check if the player is in the game
    let player = match game.pmap.get_mut(&data.fleet) {
        Some(player) => player,
        None => {
            shared.tx.send(format!("Player {} not found in game {}", data.fleet, data.gameid)).unwrap();
            return "Player not found".to_string();
        }
    };

    // Get verifying key from player
    let verifying_key = &player.verifying_key;

    // Convert signature bytes to Signature
    let signature = Signature::from_bytes(input_data.signature.as_slice().try_into().unwrap());

    // Verify the signature against the receipt data
    if verifying_key.verify(&input_data.receipt.journal.bytes.as_slice(), &signature).is_err() {
        shared.tx.send("Invalid signature in wave request".to_string()).unwrap();
        return "Invalid signature".to_string();
    }

    // Check if someone has claimed victory and timeout is active
    if let Some((claimant, claim_time)) = &game.first_victory_claim {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if current_time - claim_time < game.victory_timeout_seconds {
            let remaining_time = game.victory_timeout_seconds - (current_time - claim_time);
            shared.tx.send(format!("Cannot wave during victory claim period. {} claimed victory. {} seconds remaining to contest by clicking on 'Win' button.", claimant, remaining_time)).unwrap();
            return "Cannot wave during victory claim period".to_string();
        }
    }

    // Check if player's board hash matches the current state (current saved board hash)
    if player.current_state != data.board {
        shared.tx.send(format!("Player {}'s board hash does not match the current state in game {}", data.fleet, data.gameid)).unwrap();
        return "Board hash mismatch".to_string();
    }

    // check if the player does not have to report
    if game.next_report.is_some() {
        shared.tx.send(format!("Cannot wave until player {} has reported in game {}", game.next_report.as_ref().unwrap(), data.gameid)).unwrap();
        return format!("Cannot wave until player {} has reported", game.next_report.as_ref().unwrap()).to_string();
    }

    // Check if it's the player's turn to wave
    if game.next_player.as_ref() != Some(&data.fleet) {
        shared.tx.send(format!("Not {}'s turn to wave in game {}", data.fleet, data.gameid)).unwrap();
        return "Not your turn to wave".to_string();
    }

    // Find the player who hasn't had a turn in the longest time
    let mut oldest_timestamp = u64::MAX;
    let mut next_player_name = String::new();
    
    for (player_name, player_data) in &game.pmap {
        if player_name != &data.fleet && player_data.last_turn_timestamp < oldest_timestamp {
            oldest_timestamp = player_data.last_turn_timestamp;
            next_player_name = player_name.clone();
        }
    }
    
    if next_player_name.is_empty() {
        shared.tx.send(format!("Player {} has no other players to pass turn to in game {}", data.fleet, data.gameid)).unwrap();
        return "No other players to pass turn to".to_string();
    }
    
    // Update the next player to the one who hasn't played the longest
    game.next_player = Some(next_player_name.clone());
    
    // Send a message about the successful wave
    let msg = format!(
        "{} waved in game {} and passed turn to {} (who hasn't played since timestamp {})",
        data.fleet,
        data.gameid,
        next_player_name,
        oldest_timestamp
    );
    shared.tx.send(msg).unwrap();

    "OK".to_string()
}

fn handle_win(shared: &SharedData, input_data: &CommunicationData) -> String {
    // Verify the receipt first
    if input_data.receipt.verify(WIN_ID).is_err() {
        shared.tx.send("Attempting to win with invalid receipt".to_string()).unwrap();
        return "Could not verify receipt".to_string();
    }

    // Decode the journal
    let data: BaseJournal = input_data.receipt.journal.decode().unwrap();
    let mut gmap = shared.gmap.lock().unwrap();

    // Check if the game exists
    let game = match gmap.get_mut(&data.gameid) {
        Some(game) => game,
        None => {
            shared.tx.send(format!("Game {} not found", data.gameid)).unwrap();
            return "Game not found".to_string();
        }
    };

    // Check if the player is in the game
    let player = match game.pmap.get_mut(&data.fleet) {
        Some(player) => player,
        None => {
            shared.tx.send(format!("Player {} not found in game {}", data.fleet, data.gameid)).unwrap();
            return "Player not found".to_string();
        }
    };

    // Get verifying key from player
    let verifying_key = &player.verifying_key;

    // Convert signature bytes to Signature
    let signature = Signature::from_bytes(input_data.signature.as_slice().try_into().unwrap());

    // Verify the signature against the receipt data
    if verifying_key.verify(&input_data.receipt.journal.bytes.as_slice(), &signature).is_err() {
        shared.tx.send("Invalid signature in win request".to_string()).unwrap();
        return "Invalid signature".to_string();
    }

    // Check if player's board hash matches the current state (current saved board hash)
    if player.current_state != data.board {
        shared.tx.send(format!("Player {}'s board hash does not match the current state in game {}", data.fleet, data.gameid)).unwrap();
        return "Board hash mismatch".to_string();
    }

    // Get current timestamp
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Check if player has already claimed victory
    if player.has_claimed_victory {
        shared.tx.send(format!("Player {} has already claimed victory in game {}", data.fleet, data.gameid)).unwrap();
        return "Already claimed victory".to_string();
    }

    // Save that the player has declared victory
    player.has_claimed_victory = true;

    // Check if this is the first victory claim
    if game.first_victory_claim.is_none() {
        game.first_victory_claim = Some((data.fleet.clone(), current_time));
        let msg = format!("{} claims victory in game {}. Other players have {} seconds to contest by clicking on 'Win' button.", 
                         data.fleet, data.gameid, game.victory_timeout_seconds);
        shared.tx.send(msg).unwrap();
        return "Victory claimed - timeout started.".to_string();
    }

    // Check if we're still within the timeout period
    let (first_claimant, first_claim_time) = game.first_victory_claim.as_ref().unwrap();
    if current_time - first_claim_time < game.victory_timeout_seconds {
        let remaining_time = game.victory_timeout_seconds - (current_time - first_claim_time);
        let msg = format!("{} contests victory of player {} in game {}! Game will resume after {} seconds.", 
                         data.fleet, first_claimant, data.gameid, remaining_time);
        shared.tx.send(msg).unwrap();
        return "Victory contested. Game continues.".to_string();
    }

    // Timeout period has passed, check who won
    let all_victors: Vec<String> = game.pmap
        .iter()
        .filter(|(_, player)| player.has_claimed_victory)
        .map(|(name, _)| name.clone())
        .collect();

    if all_victors.len() == 1 {
        let winner = &all_victors[0];
        let msg = format!("Victory timeout expired. {} wins game {}! Game ended.", winner, data.gameid);
        shared.tx.send(msg).unwrap();
        
        // Clean everything and end the game
        gmap.remove(&data.gameid);
        
        return format!("{} wins - Game ended", winner);
    } else {
        let conflict_msg = format!(
            "Victory timeout expired in game {} with multiple claimants: {}. No winner declared. Game continues as normal.",
            data.gameid,
            all_victors.join(", ")
        );
        shared.tx.send(conflict_msg).unwrap();
        
        // Reset victory claims and continue the game
        for (_, player) in &mut game.pmap {
            player.has_claimed_victory = false;
        }
        game.first_victory_claim = None;
        
        return "Multiple victory claims - no winner. Game continues as normal.".to_string();
    }
}

async fn check_victory_timeouts(shared: &SharedData) {
    let mut gmap = shared.gmap.lock().unwrap();
    let mut games_to_remove = Vec::new();
    
    for (gameid, game) in gmap.iter_mut() {
        if let Some((first_claimant, first_claim_time)) = &game.first_victory_claim {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            if current_time - first_claim_time >= game.victory_timeout_seconds {
                // Handle timeout expiration logic here
                let all_victors: Vec<String> = game.pmap
                    .iter()
                    .filter(|(_, player)| player.has_claimed_victory)
                    .map(|(name, _)| name.clone())
                    .collect();

                if all_victors.len() == 1 {
                    let winner = &all_victors[0];
                    let msg = format!("Victory timeout expired. {} wins game {}! Game ended.", winner, gameid);
                    shared.tx.send(msg).unwrap();
                    games_to_remove.push(gameid.clone());
                } else {
                    let conflict_msg = format!(
                        "Victory timeout expired in game {} with multiple claimants: {}. No winner declared. Game continues as normal.",
                        gameid,
                        all_victors.join(", ")
                    );
                    shared.tx.send(conflict_msg).unwrap();
                    
                    // Reset victory claims
                    for (_, player) in &mut game.pmap {
                        player.has_claimed_victory = false;
                    }
                    game.first_victory_claim = None;
                }
            }
        }
    }
    
    // Remove ended games
    for gameid in games_to_remove {
        gmap.remove(&gameid);
    }
}
