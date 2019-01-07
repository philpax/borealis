extern crate i2cdev;

use i2cdev::core::*;
use i2cdev::linux::{LinuxI2CDevice, LinuxI2CError};

struct Device(LinuxI2CDevice);
#[derive(Copy, Clone)]
struct Colour(u8, u8, u8);

impl Device {
    fn write_register(&mut self, reg: u8, byte: u8) -> Result<(), LinuxI2CError> {
        let register = ((reg & 0xFF) as u16) << 8 | 0x0080;
        self.0.smbus_write_word_data(0x00, register)?;
        self.0.smbus_write_byte_data(0x01, byte)?;
        Ok(())
    }

    fn enable(&mut self) -> Result<(), LinuxI2CError> {
        self.write_register(0xF8, 0x00)?;
        self.write_register(0xF9, 0xE0)?;
        Ok(())
    }

    fn enable_individual(&mut self) -> Result<(), LinuxI2CError> {
        self.write_register(0xF8, 0x01)?;
        self.write_register(0xF9, 0xE2)?;

        self.write_register(0xF8, 0x00)?;
        self.write_register(0xF9, 0xE0)?;
        Ok(())
    }

    fn static_mode(&mut self) -> Result<(), LinuxI2CError> {
        self.write_register(0x21, 0x01)
    }

    fn set_colour(&mut self, colours: &[Colour]) -> Result<(), LinuxI2CError> {
        let mut data = vec![];
        for colour in colours {
            data.extend([colour.0, colour.2, colour.1].iter());
        }
        self.0.smbus_write_word_data(0x00, 0x0080)?;
        self.0.smbus_write_block_data(0x03, data.as_slice())?;
        self.write_register(0xA0, 0x01)?;
        Ok(())
    }
}

fn main() -> Result<(), LinuxI2CError> {
    let mut dev1 = Device(LinuxI2CDevice::new("/dev/i2c-1", 0x77)?);
    dev1.enable_individual()?;

    let dev2 = Device(LinuxI2CDevice::new("/dev/i2c-1", 0x71)?);

    let mut devs = [dev1, dev2];

    let colour = Colour(0, 0, 128);

    let colours = vec![colour, colour, colour, colour, colour];
    for dev in devs.iter_mut() {
        dev.static_mode()?;
        dev.set_colour(colours.as_slice())?;
    }

    Ok(())
}
