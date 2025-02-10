// src/game_actions.rs

use fleetcore::{BaseInputs, Command, FireInputs};
use methods::{FIRE_ELF, JOIN_ELF, REPORT_ELF, WAVE_ELF, WIN_ELF};
use crate::{unmarshal_data, unmarshal_fire, unmarshal_report, send_receipt, FormData};

pub async fn join_game(idata: FormData) -> String {
    let (gameid, fleetid, board, random) = match unmarshal_data(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };

    // TO DO: Rebuild the receipt 
    let receipt = none; 

    // Then send it
    send_receipt(Command::Join, receipt).await
}

pub async fn fire(idata: FormData) -> String {
    let (gameid, fleetid, board, random, targetfleet, x, y) = match unmarshal_fire(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };
    // TO DO: Rebuild the receipt 
    let receipt = none; 
    send_receipt(Command::Fire, receipt).await
}

pub async fn report(idata: FormData) -> String {
    let (gameid, fleetid, board, random, report, x, y) = match unmarshal_report(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };
    // TO DO: Rebuild the receipt 
    let receipt = none; 
    send_receipt(Command::Fire, receipt).await
}

pub async fn wave(idata: FormData) -> String {
    let (gameid, fleetid, board, random) = match unmarshal_data(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };
    // TO DO: Rebuild the receipt 
    let receipt = none; 
    send_receipt(Command::Wave, receipt).await
}

pub async fn win(idata: FormData) -> String {
    let (gameid, fleetid, board, random) = match unmarshal_data(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };
    // TO DO: Rebuild the receipt 
    let receipt = none; 
    send_receipt(Command::Win, receipt).await
}