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

use fleetcore::{BaseJournal, Command, FireJournal, CommunicationData, ReportJournal};
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
        shared.tx.send("Attempting to join game with invalid receipt".to_string()).unwrap();
        return "Could not verify receipt".to_string();
    }
    let data: BaseJournal = input_data.receipt.journal.decode().unwrap();
    let mut gmap = shared.gmap.lock().unwrap();
    let game = gmap.entry(data.gameid.clone()).or_insert(Game {
        pmap: HashMap::new(),
        next_player: Some(data.fleet.clone()),
        next_report: None,
    });
    let player_inserted = game.pmap.entry(data.fleet.clone()).or_insert_with(|| Player {
        name: data.fleet.clone(),
        current_state: data.board.clone(),
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
    // Check validity of receipt
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

    // Check if the player is in the game
    let player = match game.pmap.get_mut(&data.fleet) {
        Some(player) => player,
        None => {
            shared.tx.send(format!("Player {} not found in game {}", data.fleet, data.gameid)).unwrap();
            return "Player not found".to_string();
        }
    };

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

    // Check if the target is not the player itself
    if data.fleet == data.target {
        shared.tx.send(format!("Cannot fire at yourself in game {}", data.gameid)).unwrap();
        return "Cannot fire at yourself".to_string();
    }

    // Check if the target is in the game
    if !game.pmap.contains_key(&data.target) {
        shared.tx.send(format!("Target {} not found in game {}", data.target, data.gameid)).unwrap();
        return "Target not found".to_string();
    }

    // Update who needs to report to the player that was just fired at
    game.next_report = Some(data.target.clone());
    
    // Update the next player (next_player will be attributed to the player that was just fired at after they report)
    game.next_player = None;
    
    // Send a message about the successful shot
    let msg  = format!(
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
    // TO DO:
    "OK".to_string()
}

fn handle_wave(shared: &SharedData, input_data: &CommunicationData) -> String {
    // TO DO:
    "OK".to_string()
}

fn handle_win(shared: &SharedData, input_data: &CommunicationData) -> String {
    // TO DO:
    "OK".to_string()
}
