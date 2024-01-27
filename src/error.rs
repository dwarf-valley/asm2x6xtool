/*
 *  asm2x6xtool - configuration and firmware management for ASM2x6x chips
 *  Copyright (C) 2024 Sven Peter <sven@svenpeter.dev>
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, version 3.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 *  along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum Error {
    USB(rusb::Error),
    InvalidCDB,
    InvalidCSW,
    CSWIOError(u8),
    TransferStillPending,
    InvalidCSWTag,
    NoTransferPending,
    CSWResidue(u32),
    IO(std::io::Error),
    #[cfg(target_os = "linux")]
    Nix(nix::Error),
    #[cfg(target_os = "linux")]
    SgIoError,
}

impl std::error::Error for Error {}

impl From<rusb::Error> for Error {
    fn from(err: rusb::Error) -> Self {
        Error::USB(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IO(err)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::USB(err) => write!(f, "USB error: {}", err),
            Error::InvalidCDB => write!(f, "Invalid arguments to create CDW"),
            Error::InvalidCSW => write!(f, "Invalid CSW signature"),
            Error::CSWIOError(io) => write!(f, "CSW I/O error: {}", io),
            Error::TransferStillPending => write!(f, "Transfer still pending"),
            Error::InvalidCSWTag => write!(f, "Invalid CSW tag"),
            Error::NoTransferPending => write!(f, "No transfer pending"),
            Error::CSWResidue(residue) => write!(f, "CSW residue > 0: {}", residue),
            Error::IO(err) => write!(f, "IO error: {}", err),
            #[cfg(target_os = "linux")]
            Error::Nix(err) => write!(f, "Nix error: {}", err),
            #[cfg(target_os = "linux")]
            Error::SgIoError => write!(f, "SG_IO ioctl failed"),
        }
    }
}
