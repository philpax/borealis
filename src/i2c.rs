use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn find_smbus() -> io::Result<PathBuf> {
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

pub struct I2CAdapter {
    pub path: PathBuf,
    pub port: u8,
    pub base_address: u32,
}

pub fn find_i2c_adapters<P: AsRef<Path>>(smbus_path: P) -> io::Result<Vec<I2CAdapter>> {
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
