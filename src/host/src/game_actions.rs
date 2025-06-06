// src/game_actions.rs

use fleetcore::{BaseInputs, Command, FireInputs};
use methods::{FIRE_ELF, JOIN_ELF, REPORT_ELF, WAVE_ELF, WIN_ELF};
use ed25519_dalek::Signer;

use crate::{
    generate_receipt_for_base_inputs, send_receipt, unmarshal_data, unmarshal_fire,
    unmarshal_report, FormData, generate_receipt_for_fire_inputs, generate_keys_from_random,
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
        Ok(receipt) => {
            // Generate keys from the random string
            let (signing_key, verifying_key) = generate_keys_from_random(&random);

            // Sign the receipt with the generated key
            let signature = signing_key.sign(&receipt.journal.bytes.as_slice()).to_bytes();
            let public_key = verifying_key.to_bytes();

            // Send the receipt along with the command and keys
            send_receipt(Command::Join, receipt, &signature, Some(&public_key)).await
        }
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
        Ok(receipt) => {
            // Generate keys from the random string
            let (signing_key, _verifying_key) = generate_keys_from_random(&random);

            // Sign the receipt with the generated key
            let signature = signing_key.sign(&receipt.journal.bytes.as_slice()).to_bytes();

            // Send the receipt along with the command and keys
            send_receipt(Command::Fire, receipt, &signature, None).await
        }
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
        Ok(receipt) => {
            // Generate keys from the random string
            let (signing_key, _verifying_key) = generate_keys_from_random(&random);

            // Sign the receipt with the generated key
            let signature = signing_key.sign(&receipt.journal.bytes.as_slice()).to_bytes();

            // Send the receipt along with the command and keys
            send_receipt(Command::Report, receipt, &signature, None).await
        }
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
        Ok(receipt) => {
            // Generate keys from the random string
            let (signing_key, _verifying_key) = generate_keys_from_random(&random);

            // Sign the receipt with the generated key
            let signature = signing_key.sign(&receipt.journal.bytes.as_slice()).to_bytes();

            // Send the receipt along with the command and keys
            send_receipt(Command::Wave, receipt, &signature, None).await
        }
        Err(e) => format!("Error creating wave receipt: {}.", e),
    }
}

pub async fn win(idata: FormData) -> String {
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

    match generate_receipt_for_base_inputs(base_inputs, WIN_ELF) {
        Ok(receipt) => {
            // Generate keys from the random string
            let (signing_key, _verifying_key) = generate_keys_from_random(&random);

            // Sign the receipt with the generated key
            let signature = signing_key.sign(&receipt.journal.bytes.as_slice()).to_bytes();

            // Send the receipt along with the command and keys
            send_receipt(Command::Win, receipt, &signature, None).await
        }
        Err(e) => format!("Error creating win receipt: {}.", e),
    }
}
