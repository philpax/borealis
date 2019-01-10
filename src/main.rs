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

    const AURA_TRIDENT_Z_START: u8 = 0x70;
    const AURA_TRIDENT_Z_END: u8 = 0x77;
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
        AuraController::connect("MB", &i2c_aux.path, AURA_MB_ADDR).expect("Can't connect to Aura MB controller. If using an AMD system, have you applied the kernel patch?"),
    ];

    for addr in AURA_TRIDENT_Z_START..=AURA_TRIDENT_Z_END {
        if let Ok(controller) = AuraController::connect(&format!("RAM{:x}", addr), &i2c_sys.path, addr) {
            controllers.push(controller);
        }
    }

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
