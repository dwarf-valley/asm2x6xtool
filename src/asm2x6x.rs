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

use crate::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;
use std::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    ASM2464PD,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    ConfigRead = 0xe0,
    ConfigWrite = 0xe1,
    FlashRead = 0xe2,
    FlashWrite = 0xe3,
    Read = 0xe4,
    Write = 0xe5,
    Reload = 0xe8,
}

pub trait Backend {
    fn model(&self) -> Model;

    fn transfer(&mut self, cdb: &[u8]) -> Result<(), Error>;
    fn transfer_to_device(&mut self, cdb: &[u8], data: &[u8]) -> Result<(), Error>;
    fn transfer_from_device(&mut self, cdb: &[u8], data: &mut [u8]) -> Result<(), Error>;
}

pub struct Device {
    backend: Box<dyn Backend>,
}

pub struct FWVersion {
    day: u8,
    month: u8,
    year: u8,
    major: u8,
    minor: u8,
    patch: u8,
}

impl Display for FWVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:02x}{:02x}{:02x}_{:02x}_{:02x}_{:02x}",
            self.year, self.month, self.day, self.major, self.minor, self.patch
        )
    }
}

impl Device {
    pub fn new(backend: Box<dyn Backend>) -> Self {
        Self { backend }
    }

    pub fn read(&mut self, mut addr: u32, bfr: &mut [u8]) -> Result<(), Error> {
        let mut cdb = [0_u8; 6];

        addr &= 0x01ffff;
        addr |= 0x500000;

        cdb[0] = Command::Read as u8;
        cdb[1] = bfr.len() as u8;
        cdb[2] = (addr >> 16) as u8;
        cdb[3] = (addr >> 8) as u8;
        cdb[4] = addr as u8;
        cdb[5] = 0x00;

        self.backend.transfer_from_device(&cdb, bfr)
    }

    pub fn write(&mut self, mut addr: u32, value: u8) -> Result<(), Error> {
        let mut cdb = [0_u8; 6];

        addr &= 0x01ffff;
        addr |= 0x500000;

        cdb[0] = Command::Write as u8;
        cdb[1] = value;
        cdb[2] = (addr >> 16) as u8;
        cdb[3] = (addr >> 8) as u8;
        cdb[4] = addr as u8;
        cdb[5] = 0x00;

        self.backend.transfer(&cdb)
    }

    pub fn read_fw_version(&mut self) -> Result<FWVersion, Error> {
        let mut bfr = [0_u8; 6];
        self.read(0x07f0, &mut bfr)?;

        Ok(FWVersion {
            year: bfr[0],
            month: bfr[1],
            day: bfr[2],
            major: bfr[3],
            minor: bfr[4],
            patch: bfr[5],
        })
    }

    pub fn read_config(&mut self) -> Result<[u8; 0x80], Error> {
        let cdb = [Command::ConfigRead as u8, 0x50, 0x00, 0x00, 0x00, 0x00];
        let mut bfr = [0_u8; 0x80];
        self.backend.transfer_from_device(&cdb, &mut bfr)?;
        Ok(bfr)
    }

    pub fn read_firmware(&mut self) -> Result<Vec<u8>, Error> {
        let mut bfr = vec![0_u8; 0x17ee0];

        let mut cdb = [Command::FlashRead as u8, 0x00, 0x00, 0x00, 0x00, 0x00];

        // first part, 0x0 to 0xff00
        cdb[1] = 0x50;
        cdb[2] = 0x00;
        cdb[3] = 0xff;
        cdb[4] = 0x00;
        self.backend
            .transfer_from_device(&cdb, &mut bfr[..0xff00])?;

        // the device sometimes dies if the next transfer is requested too quickly
        std::thread::sleep(std::time::Duration::from_millis(1000));

        // second part, 0xff00 - 0x17ee0
        cdb[1] = 0xd0;
        cdb[2] = 0x00;
        cdb[3] = 0x7f;
        cdb[4] = 0xe0;
        self.backend
            .transfer_from_device(&cdb, &mut bfr[0xff00..])?;

        // the device sometimes dies if the next transfer is requested too quickly
        std::thread::sleep(std::time::Duration::from_millis(1000));

        Ok(bfr)
    }
}
