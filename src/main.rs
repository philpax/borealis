extern crate i2cdev;

use i2cdev::core::*;
use i2cdev::linux::LinuxI2CDevice;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn find_smbus() -> io::Result<PathBuf> {
    #[allow(clippy::large_digit_groups)]
    const SMBUS_CLASS: u32 = 0x000_c0500;
    for entry in fs::read_dir("/sys/bus/pci/devices")? {
        let entry = entry?;
        let path = entry.path();

        let class_path = path.join("class");
        let class_buf = fs::read(class_path)?;
        let class_str = String::from_utf8_lossy(&class_buf);
        let class_stripped_str = &class_str[2..class_str.len()].trim();
        let class = u32::from_str_radix(class_stripped_str, 16).expect("failed to parse class");

        if class == SMBUS_CLASS {
            return Ok(path);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "failed to find smbus",
    ))
}

struct I2CAdapter {
    path: PathBuf,
    port: u8,
    base_address: u32,
}

fn find_i2c_adapters<P: AsRef<Path>>(smbus_path: P) -> io::Result<Vec<I2CAdapter>> {
    let mut ret = vec![];
    for entry in fs::read_dir(smbus_path)? {
        let entry = entry?;
        let path = entry.path();
        let filename = path
            .file_name()
            .expect("expected filename while iterating smbus")
            .to_string_lossy();

        if !filename.starts_with("i2c-") {
            continue;
        }

        let name_path = path.join("name");
        let name_buf = fs::read(name_path)?;
        let name_str = String::from_utf8_lossy(&name_buf);
        let name_components: Vec<_> = name_str.trim().split(' ').collect();
        let port: u8 = name_components[name_components.len() - 3]
            .parse()
            .expect("failed to parse port");
        let port_base_address = u32::from_str_radix(
            name_components
                .last()
                .expect("expected components in i2c name"),
            16,
        )
        .expect("failed to parse port base address");

        ret.push(I2CAdapter {
            path: ["/dev", &filename].iter().collect(),
            port,
            base_address: port_base_address,
        });
    }

    Ok(ret)
}

struct Ec3572(LinuxI2CDevice);
impl Ec3572 {
    fn connect<P: AsRef<Path>>(i2c_path: P, address: u8) -> Option<Self> {
        Some(Ec3572(
            LinuxI2CDevice::new(i2c_path, u16::from(address)).ok()?,
        ))
    }

    fn translate_register(register: u8) -> u16 {
        (u16::from(register) << 8) | 0x0080
    }

    fn read_register_byte(&mut self, register: u8) -> Option<u8> {
        self.0
            .smbus_write_word_data(0x00, Self::translate_register(register))
            .ok()?;
        self.0.smbus_read_byte_data(0x81).ok()
    }

    fn write_register_byte(&mut self, register: u8, val: u8) -> Option<()> {
        self.0
            .smbus_write_word_data(0x00, Self::translate_register(register))
            .ok()?;
        self.0.smbus_write_byte_data(0x01, val).ok()
    }

    fn set_colours(&mut self, colours: &[u8]) -> Option<()> {
        self.0
            .smbus_write_word_data(0x00, Self::translate_register(0))
            .ok()?;
        self.0.smbus_write_block_data(0x03, colours).ok()
    }
}

struct AuraController {
    name: String,
    ec3572: Ec3572,
    led_counts: Vec<usize>,
    total_led_count: usize,
}

impl AuraController {
    // Registers
    const REGISTER_COUNT: u8 = 0xC1;
    const LED_COUNT_BASE: u8 = 0xC8;
    const ASSERT_UPLOAD: u8 = 0xA0;

    fn connect<P: AsRef<Path>>(name: &str, i2c_path: P, address: u8) -> Option<Self> {
        let name = name.to_string();
        let ec3572 = Ec3572::connect(i2c_path, address)?;

        let mut controller = AuraController {
            name,
            ec3572,
            led_counts: vec![],
            total_led_count: 0,
        };
        controller.initialize()?;

        Some(controller)
    }

    fn initialize(&mut self) -> Option<()> {
        // Find number of LEDs.
        let register_count = self.register_count()?;
        for i in 0..register_count {
            let led_count = self
                .ec3572
                .read_register_byte(AuraController::LED_COUNT_BASE + i)?
                as usize;
            self.led_counts.push(led_count);
            self.total_led_count += led_count & 0xF;
        }

        println!("{}: total led count {}", self.name, self.total_led_count);

        // Initialize in static mode.
        self.ec3572.write_register_byte(0x20, 0x01)?;
        self.ec3572.write_register_byte(0x21, 0x0F)?;
        self.ec3572.write_register_byte(0x25, 0xFF)?;

        Some(())
    }

    fn register_count(&mut self) -> Option<u8> {
        self.ec3572
            .read_register_byte(AuraController::REGISTER_COUNT)
    }

    fn total_led_count(&self) -> usize {
        self.total_led_count
    }

    fn set_colours(&mut self, colours: &[u8]) -> Option<()> {
        if colours.len() % 3 != 0 {
            // TODO: Result would be nice here!
            return None;
        }

        self.ec3572.set_colours(&colours)?;
        self.ec3572
            .write_register_byte(AuraController::ASSERT_UPLOAD, 0x01)
    }
}

fn main() -> io::Result<()> {
    const AMD_SMBUS_PORT_BASE_ADDRESS: u32 = 0xB00;
    const AMD_AURA_PORT_BASE_ADDRESS: u32 = 0xB20;

    const AURA_TRIDENT_Z_ADDR_1: u8 = 0x70;
    const AURA_TRIDENT_Z_ADDR_2: u8 = 0x71;
    const AURA_TRIDENT_Z_ADDR_3: u8 = 0x73;
    const AURA_TRIDENT_Z_ADDR_4: u8 = 0x74;
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

    let mut controllers = vec![
        AuraController::connect("RAM1", &i2c_sys.path, AURA_TRIDENT_Z_ADDR_1).unwrap(),
        AuraController::connect("RAM2", &i2c_sys.path, AURA_TRIDENT_Z_ADDR_2).unwrap(),
        AuraController::connect("RAM3", &i2c_sys.path, AURA_TRIDENT_Z_ADDR_3).unwrap(),
        AuraController::connect("RAM4", &i2c_sys.path, AURA_TRIDENT_Z_ADDR_4).unwrap(),
        AuraController::connect("MB", &i2c_aux.path, AURA_MB_ADDR).unwrap(),
    ];

    // TODO: Implement additional Ec3572 probing (0xAA to 0xAD inclusive on ec3572 apparently
    // point to additional Ec3572 ports; would then need to translate that into I2C)

    for controller in controllers.iter_mut() {
        let colours: Vec<u8> = vec![127; controller.total_led_count() * 3];
        controller.set_colours(&colours);
    }

    Ok(())
}
