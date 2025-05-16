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

use fleetcore::{BaseJournal, Command, CommunicationData, FireJournal, ReportJournal};
use methods::{FIRE_ID, JOIN_ID, REPORT_ID, WAVE_ID, WIN_ID};

struct Player {
    name: String,
    current_state: Digest,
}
struct Game {
    pmap: HashMap<String, Player>,
    next_player: Option<String>,
    next_report: Option<String>,
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
    if input_data.receipt.verify(JOIN_ID).is_err() {
        shared
            .tx
            .send("Attempting to join game with invalid receipt".to_string())
            .unwrap();
        return "Could not verify receipt".to_string();
    }
    let data: BaseJournal = input_data.receipt.journal.decode().unwrap();
    let mut gmap = shared.gmap.lock().unwrap();
    let game = gmap.entry(data.gameid.clone()).or_insert(Game {
        pmap: HashMap::new(),
        next_player: Some(data.fleet.clone()),
        next_report: None,
    });
    let player_inserted = game
        .pmap
        .entry(data.fleet.clone())
        .or_insert_with(|| Player {
            name: data.fleet.clone(),
            current_state: data.board.clone(),
        })
        .name
        == data.fleet;
    let mesg = if player_inserted {
        format!("Joined game {}", data.gameid)
    } else {
        format!("Player already in game {}", data.gameid)
    };
    shared.tx.send(mesg).unwrap();
    "OK".to_string()
}

fn handle_fire(shared: &SharedData, input_data: &CommunicationData) -> String {
    // Verify the receipt first
    if input_data.receipt.verify(FIRE_ID).is_err() {
        shared
            .tx
            .send("Attempting to fire with invalid receipt".to_string())
            .unwrap();
        return "Could not verify receipt".to_string();
    }

    // Decode the journal data from the receipt
    let data: FireJournal = input_data.receipt.journal.decode().unwrap();
    let mut gmap = shared.gmap.lock().unwrap();

    // Check if the game exists
    let game = match gmap.get_mut(&data.gameid) {
        Some(game) => game,
        None => {
            shared
                .tx
                .send(format!("Game {} not found", data.gameid))
                .unwrap();
            return "Game not found".to_string();
        }
    };

    // Check if it's the player's turn to fire
    if game.next_player != Some(data.fleet.clone()) {
        shared
            .tx
            .send(format!("Not {}'s turn to fire", data.fleet))
            .unwrap();
        return "Not your turn".to_string();
    }

    // Check if the target player exists in the game
    if !game.pmap.contains_key(&data.target) {
        shared
            .tx
            .send(format!("Target {} not found in game", data.target))
            .unwrap();
        return "Target not found".to_string();
    }

    // Update game state - target player needs to report the result
    game.next_player = None;
    game.next_report = Some(data.target.clone());

    // Broadcast the fire action
    shared
        .tx
        .send(format!(
            "{} fired at {} at position {}",
            data.fleet,
            data.target,
            xy_pos(data.pos)
        ))
        .unwrap();

    "OK".to_string()
}

fn handle_report(shared: &SharedData, input_data: &CommunicationData) -> String {
    // Verify the receipt
    if input_data.receipt.verify(REPORT_ID).is_err() {
        shared
            .tx
            .send("Attempting to report with invalid receipt".to_string())
            .unwrap();
        return "Could not verify receipt".to_string();
    }

    // Decode the journal data
    let data: ReportJournal = input_data.receipt.journal.decode().unwrap();
    let mut gmap = shared.gmap.lock().unwrap();

    // Check if the game exists
    let game = match gmap.get_mut(&data.gameid) {
        Some(game) => game,
        None => {
            shared
                .tx
                .send(format!("Game {} not found", data.gameid))
                .unwrap();
            return "Game not found".to_string();
        }
    };

    // Check if it's this player's turn to report
    if game.next_report != Some(data.fleet.clone()) {
        shared
            .tx
            .send(format!("Not {}'s turn to report", data.fleet))
            .unwrap();
        return "Not your turn to report".to_string();
    }

    // Update player's board state
    if let Some(player) = game.pmap.get_mut(&data.fleet) {
        player.current_state = data.next_board;
    }

    // Reset report flag and set the next player
    game.next_report = None;

    // Give the turn to the player who just reported (the one who was shot at)
    game.next_player = Some(data.fleet.clone());

    // Broadcast the report result
    let hit_status = if data.report == "Hit" { "HIT" } else { "MISS" };
    shared
        .tx
        .send(format!(
            "{} reports {} at position {}",
            data.fleet,
            hit_status,
            xy_pos(data.pos)
        ))
        .unwrap();

    "OK".to_string()
}

fn handle_wave(shared: &SharedData, input_data: &CommunicationData) -> String {
    // Verify receipt
    if input_data.receipt.verify(WAVE_ID).is_err() {
        shared
            .tx
            .send("Attempting to wave with invalid receipt".to_string())
            .unwrap();
        return "Could not verify receipt".to_string();
    }

    // Decode journal data
    let data: BaseJournal = input_data.receipt.journal.decode().unwrap();
    let mut gmap = shared.gmap.lock().unwrap();

    // Check if game exists
    let game = match gmap.get_mut(&data.gameid) {
        Some(game) => game,
        None => {
            shared
                .tx
                .send(format!("Game {} not found", data.gameid))
                .unwrap();
            return "Game not found".to_string();
        }
    };

    // Check if player is in the game
    if !game.pmap.contains_key(&data.fleet) {
        shared
            .tx
            .send(format!("Player {} not in game {}", data.fleet, data.gameid))
            .unwrap();
        return "Player not in game".to_string();
    }

    // Check if it's the player's turn
    if game.next_player != Some(data.fleet.clone()) {
        shared
            .tx
            .send(format!(
                "{} cannot wave when it's not their turn",
                data.fleet
            ))
            .unwrap();
        return "Not your turn to wave".to_string();
    }

    // Choose the next player randomly
    let mut rng = shared.rng.lock().unwrap();
    let players: Vec<&String> = game.pmap.keys().filter(|k| **k != data.fleet).collect();

    if let Some(next_player) = players.iter().choose(&mut *rng) {
        game.next_player = Some((*next_player).clone());
    } else {
        // If no other players, keep turn with current player
        game.next_player = Some(data.fleet.clone());
    }

    // Broadcast the wave action
    shared
        .tx
        .send(format!("{} waves in game {}", data.fleet, data.gameid))
        .unwrap();

    "OK".to_string()
}

fn handle_win(shared: &SharedData, input_data: &CommunicationData) -> String {
    // Verify receipt
    if input_data.receipt.verify(WIN_ID).is_err() {
        shared
            .tx
            .send("Attempting to claim win with invalid receipt".to_string())
            .unwrap();
        return "Could not verify receipt".to_string();
    }

    // Decode journal data
    let data: BaseJournal = input_data.receipt.journal.decode().unwrap();
    let mut gmap = shared.gmap.lock().unwrap();

    // Check if game exists
    let game = match gmap.get(&data.gameid) {
        Some(game) => game,
        None => {
            shared
                .tx
                .send(format!("Game {} not found", data.gameid))
                .unwrap();
            return "Game not found".to_string();
        }
    };

    // Check if player is in the game
    if !game.pmap.contains_key(&data.fleet) {
        shared
            .tx
            .send(format!("Player {} not in game {}", data.fleet, data.gameid))
            .unwrap();
        return "Player not in game".to_string();
    }

    // Remove the game from the map since it's over
    gmap.remove(&data.gameid);

    // Broadcast the win
    shared
        .tx
        .send(format!(
            "{} claims victory in game {}!",
            data.fleet, data.gameid
        ))
        .unwrap();

    "OK".to_string()
}
