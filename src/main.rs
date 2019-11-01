extern crate i2cdev;
extern crate rlua;

use std::collections::HashMap;
use std::fs;
use std::sync::mpsc::channel;
use std::thread;
use std::time;

mod aura_error;
use crate::aura_error::*;

mod i2c;
use crate::i2c::*;

mod aura_controller;
use crate::aura_controller::*;

use rlua::Lua;

fn main() -> AuraResult<()> {
    const AMD_SMBUS_PORT_BASE_ADDRESS: u32 = 0xB00;
    const AMD_AURA_PORT_BASE_ADDRESS: u32 = 0xB20;

    const AURA_TRIDENT_Z_START: u8 = 0x70;
    const AURA_TRIDENT_Z_END: u8 = 0x77;
    const AURA_MB_ADDR: u8 = 0x4E;

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

    let mut controllers_by_name: HashMap<String, AuraController> = HashMap::new();
    controllers_by_name.insert("MB".to_owned(), AuraController::connect("MB", &i2c_aux.path, AURA_MB_ADDR).expect("Can't connect to Aura MB controller. If using an AMD system, have you applied the kernel patch?"));

    for addr in AURA_TRIDENT_Z_START..=AURA_TRIDENT_Z_END {
        if let Ok(mut controller) =
            AuraController::connect(&format!("RAM{:x}", addr), &i2c_sys.path, addr)
        {
            controllers_by_name.insert(controller.name().to_owned(), controller);
        }
    }

    let lua = Lua::new();
    let led_counts_by_name: HashMap<String, usize> = controllers_by_name
        .iter()
        .map(|(k, v)| (k.clone(), v.total_led_count()))
        .collect();
    let (tx, rx) = channel();

    lua.context(move |ctx| -> rlua::Result<()> {
        let globals = ctx.globals();

        let controllers_table = ctx.create_table()?;
        for (name, count) in led_counts_by_name.iter() {
            controllers_table.set(name.clone(), *count)?;
        }
        globals.set("controllers", controllers_table)?;

        let set_colours =
            ctx.create_function(move |_, (controller_name, colours): (String, Vec<u8>)| {
                tx.send((controller_name, colours))
                    .expect("Failed to submit colours");

                Ok(())
            })?;
        globals.set("set_colours", set_colours)?;

        let filename = "script.lua";
        let script = fs::read_to_string(filename).expect("Failed to read script");
        ctx.load(&script).set_name(filename)?.exec()?;

        Ok(())
    })
    .expect("Failed to load Lua script");

    loop {
        lua.context(|ctx| -> rlua::Result<()> {
            let globals = ctx.globals();

            let tick: Option<rlua::Function> = globals.get("tick")?;
            tick.expect("Failed to get tick function")
                .call::<_, ()>(())?;

            Ok(())
        })
        .expect("Failed to call tick");

        while let Ok((controller_name, colours)) = rx.try_recv() {
            let controller = controllers_by_name
                .get_mut(&controller_name)
                .expect("Failed to get controller");
            controller.set_colours(&colours)?;
        }

        thread::sleep(time::Duration::from_millis(30));
    }
}
