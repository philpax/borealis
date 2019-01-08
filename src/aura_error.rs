use i2cdev::linux::LinuxI2CError;

use std::error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum AuraError {
    I2CError(LinuxI2CError),
    IOError(io::Error),
    Other(String),
}

impl AuraError {
    pub fn other(s: &str) -> AuraError {
        AuraError::Other(s.to_owned())
    }
}

impl fmt::Display for AuraError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            AuraError::I2CError(ref e) => e.fmt(f),
            AuraError::IOError(ref e) => e.fmt(f),
            AuraError::Other(ref s) => write!(f, "aura error: {}", &s),
        }
    }
}

impl error::Error for AuraError {
    fn description(&self) -> &str {
        match *self {
            AuraError::I2CError(ref e) => e.description(),
            AuraError::IOError(ref e) => e.description(),
            AuraError::Other(ref s) => &s,
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            AuraError::I2CError(ref e) => Some(e),
            AuraError::IOError(ref e) => Some(e),
            AuraError::Other(_) => None,
        }
    }
}

impl From<LinuxI2CError> for AuraError {
    fn from(err: LinuxI2CError) -> AuraError {
        AuraError::I2CError(err)
    }
}

impl From<io::Error> for AuraError {
    fn from(err: io::Error) -> AuraError {
        AuraError::IOError(err)
    }
}

pub type AuraResult<T> = Result<T, AuraError>;
