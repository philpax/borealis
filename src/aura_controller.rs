use crate::aura_error::*;

use i2cdev::core::*;
use i2cdev::linux::LinuxI2CDevice;

use std::path::Path;

#[derive(Debug, PartialEq)]
enum AuraControllerType {
    Unknown,
    Other,
    AuraMB,
}

pub struct AuraController {
    name: String,
    controller_type: AuraControllerType,
    smbus: LinuxI2CDevice,
    led_counts: Vec<usize>,
    total_led_count: usize,
}

impl AuraController {
    // Registers
    const REGISTER_COUNT: u16 = 0xC1;
    const LED_COUNT_BASE: u16 = 0xC8;
    const ASSERT_UPLOAD: u16 = 0xA0;

    // Public interface
    pub fn connect<P: AsRef<Path>>(name: &str, i2c_path: P, address: u8) -> AuraResult<Self> {
        let mut controller = AuraController {
            name: name.to_string(),
            controller_type: AuraControllerType::Unknown,
            smbus: LinuxI2CDevice::new(i2c_path, u16::from(address))?,
            led_counts: vec![],
            total_led_count: 0,
        };
        controller.initialize()?;

        Ok(controller)
    }

    pub fn total_led_count(&self) -> usize {
        self.total_led_count
    }

    pub fn set_colours(&mut self, colours: &[u8]) -> AuraResult<()> {
        if colours.len() != self.total_led_count() * 3 {
            return Err(AuraError::other("invalid number of LEDs passed in!"));
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
        self.write_block_data(register, &colours_swizzled)?;
        Ok(self.write_register_byte(AuraController::ASSERT_UPLOAD, 0x01)?)
    }

    // Implementation details
    fn initialize(&mut self) -> AuraResult<()> {
        // Find out what kind of device we are.
        let identifier = self.identifier()?;
        self.controller_type = if identifier.starts_with("AUMA0-E6K5") {
            AuraControllerType::AuraMB
        } else {
            AuraControllerType::Other
        };

        // Find number of LEDs.
        let register_count = self.register_count()? as u16;
        for i in 0..register_count {
            let led_count = self.read_register_byte(AuraController::LED_COUNT_BASE + i)? as usize;
            self.led_counts.push(led_count);
            self.total_led_count += led_count & 0xF;
        }

        // Initialize in static mode.
        self.write_register_byte(0x20, 0x01)?;
        self.write_register_byte(0x21, 0x0F)?;
        self.write_register_byte(0x25, 0xFF)?;

        // Output info.
        println!(
            "{}: identifier {}, total LED count {}, type {:?}",
            self.name, identifier, self.total_led_count, self.controller_type
        );

        // TODO: Implement additional Ec3572 probing (0xAA to 0xAD inclusive on Ec3572 apparently
        // point to additional Ec3572 ports; would then need to translate that into I2C)

        Ok(())
    }

    fn write_register(&mut self, register: u16) -> AuraResult<()> {
        let translated_register = (0x8000 | register).swap_bytes();
        Ok(self
            .smbus
            .smbus_write_word_data(0x00, translated_register)?)
    }

    fn read_register_byte(&mut self, register: u16) -> AuraResult<u8> {
        self.write_register(register)?;
        Ok(self.smbus.smbus_read_byte_data(0x81)?)
    }
    
    pub fn read_register_short(&mut self, register: u16) -> AuraResult<u16> {
        self.write_register(register)?;
        Ok(self.smbus.smbus_read_word_data(0x81)?)
    }

    pub fn write_register_byte(&mut self, register: u16, val: u8) -> AuraResult<()> {
        self.write_register(register)?;
        Ok(self.smbus.smbus_write_byte_data(0x01, val)?)
    }

    fn write_block_data(&mut self, register: u16, colours: &[u8]) -> AuraResult<()> {
        self.write_register(register)?;
        Ok(self.smbus.smbus_write_block_data(0x03, colours)?)
    }

    fn register_count(&mut self) -> AuraResult<u8> {
        Ok(self.read_register_byte(AuraController::REGISTER_COUNT)?)
    }

    fn identifier(&mut self) -> AuraResult<String> {
        use std::ffi::CStr;
        self.smbus.smbus_write_word_data(0x00, 0x0010)?;
        Ok(self
            .smbus
            .smbus_read_block_data(0x80 + 0x10)
            .and_then(|u| {
                Ok(unsafe { CStr::from_ptr(u.as_ptr() as *mut i8) }
                    .to_string_lossy()
                    .into_owned())
            })?)
    }
}
