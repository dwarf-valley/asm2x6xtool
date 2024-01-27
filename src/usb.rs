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

use crate::asm2x6x::{Backend, Model};
use crate::error::Error;
use log::{debug, error, info};
use rusb::UsbContext;
use std::string::ToString;
use std::vec::IntoIter;

const ASMEDIA_VID: u16 = 0x174c;
const CBW_SIGNATURE: u32 = 0x43425355;
const CSW_SIGNATURE: u32 = 0x53425355;
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

const USBSTORAGE_RESET_REQUEST: u8 = 0xff;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub device: rusb::Device<rusb::Context>,
    pub usb_bus: u8,
    pub usb_addr: u8,
    pub model: Model,
}

impl ToString for DeviceInfo {
    fn to_string(&self) -> String {
        format!("usb:{:03}:{:03}", self.usb_bus, self.usb_addr,)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Device {
    info: DeviceInfo,
    handle: rusb::DeviceHandle<rusb::Context>,
    tag: u32,
    pending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Devices(Vec<DeviceInfo>);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum CBWDirection {
    ToDevice = 0x00,
    ToHost = 0x80,
}

#[derive(Debug, Copy, Clone)]
#[allow(clippy::upper_case_acronyms)]
struct CBW {
    tag: u32,
    length: u32,
    direction: CBWDirection,
    lun: u8,
    command_length: u8,
    command_data: [u8; 16],
}

impl From<CBW> for [u8; 31] {
    fn from(cbw: CBW) -> Self {
        let mut bfr = [0_u8; 31];

        bfr[0..4].copy_from_slice(&CBW_SIGNATURE.to_le_bytes());
        bfr[4..8].copy_from_slice(&cbw.tag.to_le_bytes());
        bfr[8..12].copy_from_slice(&cbw.length.to_le_bytes());
        bfr[12] = cbw.direction as u8;
        bfr[13] = cbw.lun;
        bfr[14] = cbw.command_length;
        bfr[15..31].copy_from_slice(&cbw.command_data);

        bfr
    }
}

#[derive(Debug, Copy, Clone)]
#[allow(clippy::upper_case_acronyms)]
struct CSW {
    tag: u32,
    residue: u32,
    status: u8,
}

impl TryFrom<&[u8; 13]> for CSW {
    type Error = Error;

    fn try_from(bfr: &[u8; 13]) -> Result<Self, Self::Error> {
        if u32::from_le_bytes([bfr[0], bfr[1], bfr[2], bfr[3]]) != CSW_SIGNATURE {
            Err(Error::InvalidCSW)
        } else {
            Ok(CSW {
                tag: u32::from_le_bytes([bfr[4], bfr[5], bfr[6], bfr[7]]),
                residue: u32::from_le_bytes([bfr[8], bfr[9], bfr[10], bfr[11]]),
                status: bfr[12],
            })
        }
    }
}

impl Devices {
    pub fn enumerate() -> Result<Self, rusb::Error> {
        let rusb_devices = rusb::Context::new()?.devices()?;
        let mut devices = vec![];

        for dev in rusb_devices.iter() {
            let desc = dev.device_descriptor()?;
            let vid = desc.vendor_id();
            let pid = desc.product_id();
            let usb_bus = dev.bus_number();
            let usb_addr = dev.address();

            debug!(
                "Bus {:03} Device {:03} ID {:04x}:{:04x}",
                dev.bus_number(),
                dev.address(),
                vid,
                pid
            );

            if vid != ASMEDIA_VID {
                continue;
            }

            if pid != 0x2463 {
                continue;
            }

            devices.push(DeviceInfo {
                device: dev,
                model: Model::ASM2464PD,
                usb_bus,
                usb_addr,
            });
        }

        Ok(Devices(devices))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl IntoIterator for Devices {
    type Item = DeviceInfo;
    type IntoIter = IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Device {
    pub fn new(info: DeviceInfo) -> Result<Self, rusb::Error> {
        let mut handle = info.device.open()?;

        if handle.kernel_driver_active(0)? {
            info!("detaching kernel driver from {:?}", info);
            if let Err(err) = handle.detach_kernel_driver(0) {
                error!("failed to detach kernel driver from {:?}: {}", info, err);
                return Err(err);
            }
        }

        debug!("claiming interface 0 on {:?}", info);
        handle.claim_interface(0)?;

        debug!("resetting usb storage interface");
        handle.write_control(
            rusb::request_type(
                rusb::Direction::Out,
                rusb::RequestType::Class,
                rusb::Recipient::Interface,
            ),
            USBSTORAGE_RESET_REQUEST,
            0,
            0,
            &[],
            TIMEOUT,
        )?;
        std::thread::sleep(std::time::Duration::from_micros(10000));
        handle.clear_halt(0x02)?;
        std::thread::sleep(std::time::Duration::from_micros(10000));
        handle.clear_halt(0x81)?;
        std::thread::sleep(std::time::Duration::from_micros(10000));

        debug!("device successfully initialized");
        Ok(Device {
            info,
            handle,
            tag: 0xdeadbeef,
            pending: false,
        })
    }

    fn send_cbw(&mut self, cdb: &[u8], direction: CBWDirection, length: u32) -> Result<(), Error> {
        if cdb.len() > 16 {
            return Err(Error::InvalidCDB);
        }

        if self.pending {
            return Err(Error::TransferStillPending);
        }

        let mut command_data = [0_u8; 16];
        command_data[..cdb.len()].copy_from_slice(cdb);

        self.tag += 1;
        let cbw: CBW = CBW {
            tag: self.tag,
            length,
            direction,
            lun: 0x00,
            command_length: cdb.len() as u8,
            command_data,
        };

        let bfr = <[u8; 31]>::from(cbw);
        debug!("Sending CBW: {:?}", bfr);

        self.handle.write_bulk(0x02, &bfr, TIMEOUT)?;
        self.pending = true;

        debug!("CBW sent successfully");
        Ok(())
    }

    fn recv_csw(&mut self) -> Result<(), Error> {
        if !self.pending {
            return Err(Error::NoTransferPending);
        }

        debug!("trying to read CSW");
        let mut bfr = [0_u8; 13];
        self.handle.read_bulk(0x81, &mut bfr, TIMEOUT)?;
        debug!("CSW: {:?}", bfr);

        let csw = CSW::try_from(&bfr)?;
        if csw.status != 0x00 {
            return Err(Error::CSWIOError(csw.status));
        }
        if csw.tag != self.tag {
            return Err(Error::InvalidCSWTag);
        }
        if csw.residue != 0 {
            return Err(Error::CSWResidue(csw.residue));
        }

        self.pending = false;
        Ok(())
    }
}

impl Backend for Device {
    fn model(&self) -> Model {
        self.info.model
    }

    fn transfer(&mut self, cdb: &[u8]) -> Result<(), Error> {
        self.send_cbw(cdb, CBWDirection::ToDevice, 0)?;
        self.recv_csw()?;
        Ok(())
    }

    fn transfer_to_device(&mut self, cdb: &[u8], data: &[u8]) -> Result<(), Error> {
        self.send_cbw(cdb, CBWDirection::ToDevice, data.len() as u32)?;

        debug!("trying to send {} bytes to device", data.len());
        self.handle.write_bulk(0x02, data, TIMEOUT)?;

        self.recv_csw()?;
        Ok(())
    }

    fn transfer_from_device(&mut self, cdb: &[u8], data: &mut [u8]) -> Result<(), Error> {
        self.send_cbw(cdb, CBWDirection::ToHost, data.len() as u32)?;

        debug!("trying to read {} bytes from device", data.len());
        self.handle.read_bulk(0x81, data, TIMEOUT)?;

        self.recv_csw()?;
        Ok(())
    }
}
