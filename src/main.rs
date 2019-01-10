extern crate i2cdev;

use std::env;
use std::iter;

mod aura_error;
use crate::aura_error::*;

mod i2c;
use crate::i2c::*;

mod aura_controller;
use crate::aura_controller::*;

fn main() -> AuraResult<()> {
    const AMD_SMBUS_PORT_BASE_ADDRESS: u32 = 0xB00;
    const AMD_AURA_PORT_BASE_ADDRESS: u32 = 0xB20;

    const AURA_TRIDENT_Z_GLOBAL: u8 = 0x77;
    // The last four are unverified. I've only tested with 4 RAM sticks.
    const AURA_TRIDENT_Z_ADDRS: [u8; 8] = [0x70, 0x71, 0x73, 0x74, 0x72, 0x75, 0x76, 0x77];
    const AURA_MB_ADDR: u8 = 0x4E;

    let args: Vec<String> = env::args().skip(1).take(3).collect();
    if args.len() != 3 {
        panic!("borealis r g b");
    }

    let cols: Vec<u8> = args
        .iter()
        .map(|c| c.parse().expect("expected integer while parsing arguments"))
        .collect();

    let smbus_path = find_smbus()?;
    println!("smbus: {}", smbus_path.to_string_lossy());
    let i2c_adapters = find_i2c_adapters(smbus_path)?;

    let i2c_sys = i2c_adapters
        .iter()
        .find(|a| a.port == 0 && a.base_address == AMD_SMBUS_PORT_BASE_ADDRESS)
        .expect("failed to locate AMD system SMBus");
    println!("i2c-sys: {}", i2c_sys.path.to_string_lossy());
    let i2c_aux = i2c_adapters
        .iter()
        .find(|a| a.base_address == AMD_AURA_PORT_BASE_ADDRESS)
        .expect("failed to locate auxiliary controller for MB Aura");
    println!("i2c-aux: {}", i2c_aux.path.to_string_lossy());

    let mut controllers = vec![
        AuraController::connect("MB", &i2c_aux.path, AURA_MB_ADDR).unwrap(),
    ];

    let enabled_sticks = 4;
    if let Ok(mut ram_controller) = AuraController::connect("RAMG", &i2c_sys.path, AURA_TRIDENT_Z_GLOBAL) {
        const RAM_ENABLE_COMMANDS: [u8; 8] = [0xE0, 0xE2, 0xE6, 0xE8, 0xEA, 0xEC, 0x9E, 0xCC];

        for i in 3..4 {
            //ram_controller.write_register_byte(0xF8, AURA_TRIDENT_Z_ADDRS[i] - 0x70)?;
            ram_controller.write_register_byte(0xF8, 0x77)?;
            ram_controller.write_register_byte(0xF9, RAM_ENABLE_COMMANDS[i])?;
        }

        controllers.push(ram_controller);
    }
/*
    for i in 0..enabled_sticks {
        let a = AURA_TRIDENT_Z_ADDRS[i];
        if !(a == 0x71 || a == 0x73) {
            continue;
        }
        controllers.push(
            AuraController::connect(&format!("RAM{}", i), &i2c_sys.path, AURA_TRIDENT_Z_ADDRS[i])?
        );
    }
*/

    for controller in controllers.iter_mut() {
        let colours: Vec<u8> = iter::repeat(&cols)
            .take(controller.total_led_count())
            .cloned()
            .flatten()
            .collect();
        controller.set_colours(&colours)?;
    }

    Ok(())
}
