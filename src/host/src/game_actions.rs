// src/game_actions.rs

use fleetcore::{BaseInputs, Command, FireInputs};
use methods::{FIRE_ELF, JOIN_ELF, REPORT_ELF, WAVE_ELF, WIN_ELF};

use crate::{
    generate_receipt_for_base_inputs, send_receipt, unmarshal_data, unmarshal_fire,
    unmarshal_report, FormData, generate_receipt_for_fire_inputs,
};

pub async fn join_game(idata: FormData) -> String {
    let (gameid, fleetid, board, random) = match unmarshal_data(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };

    let base_inputs = BaseInputs {
        gameid: gameid.clone(),
        fleet: fleetid.clone(),
        board: board.clone(),
        random: random.clone(),
    };

    match generate_receipt_for_base_inputs(base_inputs, JOIN_ELF) {
        Ok(receipt) => send_receipt(Command::Join, receipt).await,
        Err(e) => format!("Invalid fleet placement. Please check your fleet and try again. Must have 5 ships: 1x5, 2x4, 3x3, 4x2, 5x1 (number x size)."),
    }
}

pub async fn fire(idata: FormData) -> String {
    let (gameid, fleetid, board, random, targetfleet, x, y) = match unmarshal_fire(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };

    // Calculate the position from x and y (matches the reverse formula in xy_pos method in blockchain)
    let pos = y * 10 + x;

    let fire_inputs = FireInputs {
        gameid: gameid.clone(),
        fleet: fleetid.clone(),
        board: board.clone(),
        random: random.clone(),
        target: targetfleet.clone(),
        pos: pos,
    };

    match generate_receipt_for_fire_inputs(fire_inputs, FIRE_ELF) {
        Ok(receipt) => send_receipt(Command::Fire, receipt).await,
        Err(e) => format!("Error creating fire receipt: {}.", e),
    }
}

pub async fn report(idata: FormData) -> String {
    let (gameid, fleetid, board, random, _report, x, y) = match unmarshal_report(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };
    // Calculate the position from x and y (matches the reverse formula in xy_pos method in blockchain)
    let pos = y * 10 + x;

    let report_inputs = FireInputs {
        gameid: gameid.clone(),
        fleet: fleetid.clone(),
        board: board.clone(),
        random: random.clone(),
        target: _report.clone(),
        pos: pos,
    };

    match generate_receipt_for_fire_inputs(report_inputs, REPORT_ELF) {
        Ok(receipt) => send_receipt(Command::Report, receipt).await,
        Err(e) => format!("Error creating report receipt: {}.", e),
    }
}

pub async fn wave(idata: FormData) -> String {
    let (gameid, fleetid, board, random) = match unmarshal_data(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };
    
    let base_inputs = BaseInputs {
        gameid: gameid.clone(),
        fleet: fleetid.clone(),
        board: board.clone(),
        random: random.clone(),
    };

    match generate_receipt_for_base_inputs(base_inputs, WAVE_ELF) {
        Ok(receipt) => send_receipt(Command::Wave, receipt).await,
        Err(e) => format!("Error creating wave receipt: {}.", e),
    }
}

pub async fn win(idata: FormData) -> String {
    let (gameid, fleetid, board, random) = match unmarshal_data(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };
    // TO DO: Rebuild the receipt

    // Uncomment the following line when you are ready to send the receipt
    //send_receipt(Command::Fire, receipt).await
    // Comment out the following line when you are ready to send the receipt
    "OK".to_string()
}
