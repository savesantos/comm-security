// src/game_actions.rs

use fleetcore::{BaseInputs, Command, FireInputs};
use methods::{FIRE_ELF, JOIN_ELF, REPORT_ELF, WAVE_ELF, WIN_ELF};
use risc0_zkvm::{default_prover, ExecutorEnv, Receipt};

use crate::{send_receipt, unmarshal_data, unmarshal_fire, unmarshal_report, FormData};

// Add this helper function to isolate the non-Send types
fn generate_receipt_for_base_inputs(base_inputs: BaseInputs, elf: &[u8]) -> Receipt {
    let env = ExecutorEnv::builder()
        .write(&base_inputs)
        .unwrap()
        .build()
        .unwrap();

    // Get the default prover
    let prover = default_prover();

    // Produce a receipt
    prover.prove(env, elf).unwrap().receipt
}

// Similar helper for fire/report inputs
fn generate_receipt_for_fire_inputs(fire_inputs: FireInputs, elf: &[u8]) -> Receipt {
    let env = ExecutorEnv::builder()
        .write(&fire_inputs)
        .unwrap()
        .build()
        .unwrap();

    // Get the default prover
    let prover = default_prover();

    // Produce a receipt
    prover.prove(env, elf).unwrap().receipt
}

pub async fn join_game(idata: FormData) -> String {
    let (gameid, fleetid, board, random) = match unmarshal_data(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };

    // Rebuild the receipt
    let base_inputs = BaseInputs {
        gameid: gameid.clone(),
        fleet: fleetid.clone(),
        board: board.clone(),
        random: random.clone(),
    };

    // Generate receipt in a separate function
    let receipt = generate_receipt_for_base_inputs(base_inputs, JOIN_ELF);

    // Send the receipt to the blockchain
    send_receipt(Command::Join, receipt).await
}

pub async fn fire(idata: FormData) -> String {
    let (gameid, fleetid, board, random, targetfleet, x, y) = match unmarshal_fire(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };

    // Rebuild the receipt
    let fire_inputs = FireInputs {
        gameid: gameid.clone(),
        fleet: fleetid.clone(),
        board: board.clone(),
        random: random.clone(),
        target: targetfleet.clone(),
        pos: x + (y * 10), // Combine x,y into a single position value
    };

    // Generate receipt in a separate function
    let receipt = generate_receipt_for_fire_inputs(fire_inputs, FIRE_ELF);

    // Send the receipt to the blockchain
    send_receipt(Command::Fire, receipt).await
}

pub async fn report(idata: FormData) -> String {
    let (gameid, fleetid, board, random, report, x, y) = match unmarshal_report(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };
    
    // Rebuild the receipt
    let report_inputs = FireInputs {
        gameid: gameid.clone(),
        fleet: fleetid.clone(),
        board: board.clone(),
        random: random.clone(),
        target: report.clone(),
        pos: x + (y * 10),
    };

    // Generate receipt in a separate function
    let receipt = generate_receipt_for_fire_inputs(report_inputs, REPORT_ELF);

    // Send the receipt to the blockchain
    send_receipt(Command::Report, receipt).await
}

pub async fn wave(idata: FormData) -> String {
    let (gameid, fleetid, board, random) = match unmarshal_data(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };
    
    // Rebuild the receipt
    let base_inputs = BaseInputs {
        gameid: gameid.clone(),
        fleet: fleetid.clone(),
        board: board.clone(),
        random: random.clone(),
    };

    // Generate receipt in a separate function
    let receipt = generate_receipt_for_base_inputs(base_inputs, WAVE_ELF);

    // Send the receipt to the blockchain
    send_receipt(Command::Wave, receipt).await
}

pub async fn win(idata: FormData) -> String {
    let (gameid, fleetid, board, random) = match unmarshal_data(&idata) {
        Ok(values) => values,
        Err(err) => return err,
    };
    
    // Rebuild the receipt
    let base_inputs = BaseInputs {
        gameid: gameid.clone(),
        fleet: fleetid.clone(),
        board: board.clone(),
        random: random.clone(),
    };

    // Generate receipt in a separate function
    let receipt = generate_receipt_for_base_inputs(base_inputs, WIN_ELF);

    // Send the receipt to the blockchain
    send_receipt(Command::Win, receipt).await
}
