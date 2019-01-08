extern crate i2cdev;

use i2cdev::core::*;
use i2cdev::linux::{LinuxI2CDevice, LinuxI2CError};

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[rustfmt::skip]
const H: [u8; 4 * 5] = [
    1, 0, 0, 1,
    1, 0, 0, 1,
    1, 1, 1, 1,
    1, 0, 0, 1,
    1, 0, 0, 1
];

#[rustfmt::skip]
const E: [u8; 4 * 5] = [
    1, 1, 1, 1,
    1, 0, 0, 0,
    1, 1, 1, 0,
    1, 0, 0, 0,
    1, 1, 1, 1
];

#[rustfmt::skip]
const L: [u8; 4 * 5] = [
    1, 0, 0, 0,
    1, 0, 0, 0,
    1, 0, 0, 0,
    1, 0, 0, 0,
    1, 1, 1, 1
];

#[rustfmt::skip]
const O: [u8; 4 * 5] = [
    0, 1, 1, 0,
    1, 0, 0, 1,
    1, 0, 0, 1,
    1, 0, 0, 1,
    0, 1, 1, 0
];

#[rustfmt::skip]
const BLANK: [u8; 4 * 5] = [
    0, 0, 0, 0,
    0, 0, 0, 0,
    0, 0, 0, 0,
    0, 0, 0, 0,
    0, 0, 0, 0
];

#[rustfmt::skip]
const EXCLAMATION: [u8; 4 * 5] = [
    1, 0, 0, 0,
    0, 1, 0, 0,
    0, 0, 1, 0,
    0, 0, 0, 1,
    1, 1, 1, 1
];

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

    fn translate_register(register: u16) -> u16 {
        (0x8000 | register).swap_bytes()
    }

    fn write_register(&mut self, register: u16) -> Result<(), LinuxI2CError> {
        self.0
            .smbus_write_word_data(0x00, Self::translate_register(register))
    }

    fn read_register_byte(&mut self, register: u16) -> Option<u8> {
        self.write_register(register).ok()?;
        self.0.smbus_read_byte_data(0x81).ok()
    }

    fn write_register_byte(&mut self, register: u16, val: u8) -> Option<()> {
        self.write_register(register).ok()?;
        self.0.smbus_write_byte_data(0x01, val).ok()
    }

    fn set_colours(&mut self, register: u16, colours: &[u8]) -> Option<()> {
        self.write_register(register).ok()?;
        self.0.smbus_write_block_data(0x03, colours).ok()
    }

    fn identifier(&mut self) -> Option<String> {
        use std::ffi::CStr;
        self.0.smbus_write_word_data(0x00, 0x0010).ok()?;
        return self
            .0
            .smbus_read_block_data(0x80 + 0x10)
            .ok()
            .and_then(|u| {
                Some(
                    unsafe { CStr::from_ptr(u.as_ptr() as *mut i8) }
                        .to_string_lossy()
                        .into_owned(),
                )
            });
    }
}

#[derive(Debug, PartialEq)]
enum AuraControllerType {
    Other,
    AuraMB,
}

struct AuraController {
    name: String,
    identifier: String,
    controller_type: AuraControllerType,
    ec3572: Ec3572,
    led_counts: Vec<usize>,
    total_led_count: usize,
}

impl AuraController {
    // Registers
    const REGISTER_COUNT: u16 = 0xC1;
    const LED_COUNT_BASE: u16 = 0xC8;
    const ASSERT_UPLOAD: u16 = 0xA0;

    fn connect<P: AsRef<Path>>(name: &str, i2c_path: P, address: u8) -> Option<Self> {
        let name = name.to_string();
        let mut ec3572 = Ec3572::connect(i2c_path, address)?;

        let identifier = ec3572.identifier()?;
        let controller_type = if identifier.starts_with("AUMA0-E6K5") {
            AuraControllerType::AuraMB
        } else {
            AuraControllerType::Other
        };

        let mut controller = AuraController {
            name,
            identifier,
            controller_type,
            ec3572,
            led_counts: vec![],
            total_led_count: 0,
        };
        controller.initialize()?;

        Some(controller)
    }

    fn initialize(&mut self) -> Option<()> {
        // Find number of LEDs.
        let register_count = self.register_count()? as u16;
        for i in 0..register_count {
            let led_count = self
                .ec3572
                .read_register_byte(AuraController::LED_COUNT_BASE + i)?
                as usize;
            self.led_counts.push(led_count);
            self.total_led_count += led_count & 0xF;
        }

        // Initialize in static mode.
        self.ec3572.write_register_byte(0x20, 0x01)?;
        self.ec3572.write_register_byte(0x21, 0x0F)?;
        self.ec3572.write_register_byte(0x25, 0xFF)?;

        // Output info.
        println!(
            "{}: identifier {}, total LED count {}, type {:?}",
            self.name, self.identifier, self.total_led_count, self.controller_type
        );

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

        let mut colours_swizzled = colours.to_vec();
        for i in 0..colours_swizzled.len() / 3 {
            colours_swizzled.swap(3 * i + 1, 3 * i + 2);
        }

        let register = if self.controller_type == AuraControllerType::Other {
            0x0
        } else {
            0x100
        };
        self.ec3572.set_colours(register, &colours_swizzled)?;
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

    let args: Vec<String> = env::args().skip(1).take(3).collect();
    if args.len() != 3 {
        panic!("borealis r g b");
    }

    let mut cols = [0; 3];
    for (idx, arg) in args.iter().enumerate() {
        cols[idx] = arg
            .parse()
            .expect("expected integer when parsing arguments");
    }

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
        let colours: Vec<u8> = cols
            .iter()
            .cloned()
            .cycle()
            .take(controller.total_led_count() * 3)
            .collect();
        controller.set_colours(&colours);
    }

    let remap_x = [2, 3, 1, 0];
    let sequence = [
        H,
        BLANK,
        E,
        BLANK,
        L,
        BLANK,
        L,
        BLANK,
        O,
        BLANK,
        EXCLAMATION,
    ];
    let mut sequence_index = 0;
    loop {
        use std::thread;
        use std::time;
        let letter = sequence[sequence_index];

        for (idx, controller) in controllers.iter_mut().enumerate() {
            let mut colours: Vec<u8> = vec![];
            if idx < 4 {
                for j in 0..5 {
                    let j = if idx < 2 { j } else { 4 - j };
                    let on = letter[j * 4 + remap_x[idx]] == 1;
                    if letter != BLANK && letter != EXCLAMATION {
                        if on {
                            colours.push(255);
                            colours.push(255);
                            colours.push(255);
                        } else {
                            colours.push(100);
                            colours.push(0);
                            colours.push(0);
                        }
                    } else {
                        colours.push(0);
                        colours.push(0);
                        colours.push(0);
                    }
                }
            } else {
                if letter == EXCLAMATION {
                    colours.push(0);
                    colours.push(0);
                    colours.push(0);

                    colours.push(0);
                    colours.push(0);
                    colours.push(0);

                    colours.push(0);
                    colours.push(0);
                    colours.push(0);

                    colours.push(0);
                    colours.push(0);
                    colours.push(0);

                    colours.push(0);
                    colours.push(0);
                    colours.push(0);

                    colours.push(0);
                    colours.push(0);
                    colours.push(0);

                    colours.push(255);
                    colours.push(255);
                    colours.push(255);
                } else {
                    for j in 0..controller.total_led_count() {
                        colours.push(0);
                        colours.push(0);
                        colours.push(0);
                    }
                }
            }

            controller.set_colours(&colours);
        }

        thread::sleep(time::Duration::from_millis(if letter == BLANK {
            250
        } else if letter == EXCLAMATION {
            2000
        } else {
            1000
        }));
        sequence_index = (sequence_index + 1) % sequence.len();
    }

    Ok(())
}
