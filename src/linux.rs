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

use crate::asm2x6x::{Backend, Info, Model};
use crate::error::Error;
use log::{debug, error};
use nix::convert_ioctl_res;
use nix::libc::ioctl;
use std::ffi::c_void;
use std::fs;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

mod sg {
    #![allow(dead_code)]
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]

    include!(concat!(env!("OUT_DIR"), "/sg.rs"));

    pub const SG_INTERFACE_ID_ORIG: i32 = 'S' as i32;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub path: String,
    pub model: Model,
}

#[derive(Debug)]
struct Device {
    info: DeviceInfo,
    fd: std::fs::File,
}

enum TransferBuffer<'a> {
    None,
    ToDevice(&'a [u8]),
    FromDevice(&'a mut [u8]),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Devices(Vec<DeviceInfo>);

fn file_starts_with(base: &PathBuf, fname: &str, start: &str) -> bool {
    let path = base.as_path().join(fname);
    if !path.exists() {
        return false;
    }

    let contents = match fs::read_to_string(path.clone()) {
        Ok(s) => s.trim().to_string(),
        Err(e) => {
            debug!("failed to read {}: {}", path.display(), e);
            return false;
        }
    };

    debug!("{}: {}", fname, contents);
    contents.starts_with(start)
}

pub fn find_devices(devices: &mut Vec<Box<dyn Info>>) -> Result<(), Error> {
    for path in fs::read_dir("/sys/bus/scsi/devices/")?
        .into_iter()
        .filter_map(|dev| dev.ok().and_then(|dev| Some(dev.path())))
    {
        debug!("found scsi device candidate {:?}", path);

        if !file_starts_with(&path, "vendor", "ASMT") {
            debug!("  vendor is not ASMedia");
            continue;
        }

        if !file_starts_with(&path, "model", "ASM246X") {
            debug!("  model is not ASM246X");
            continue;
        }

        let path_scsi_generic = path.as_path().join("scsi_generic");
        if !path_scsi_generic.exists() {
            debug!("  scsi_generic does not exist");
            continue;
        }

        for sg_x in fs::read_dir(path_scsi_generic)?
            .into_iter()
            .filter_map(|dev| dev.ok())
            .filter_map(|dev| dev.file_name().into_string().ok())
            .filter(|path| path.starts_with("sg"))
        {
            let path = format!("/dev/{}", sg_x);
            let info = DeviceInfo {
                path,
                model: Model::ASM2464PD,
            };

            debug!("found device {:?}", info);
            devices.push(Box::new(info));
        }
    }

    Ok(())
}

impl ToString for DeviceInfo {
    fn to_string(&self) -> String {
        format!("sg:{}", self.path)
    }
}

impl Info for DeviceInfo {
    fn open(&self) -> Result<Box<dyn Backend>, Error> {
        let path = Path::new(&self.path);
        let fd = std::fs::OpenOptions::new().read(true).open(path)?;

        Ok(Box::new(Device {
            info: self.clone(),
            fd,
        }))
    }

    fn model(&self) -> Model {
        self.model
    }
}

impl Device {
    fn ioctl_sg_io(&mut self, cdb: &[u8], xfer: TransferBuffer) -> Result<(), Error> {
        let mut sgbuf: sg::sg_io_hdr = Default::default();
        sgbuf.interface_id = sg::SG_INTERFACE_ID_ORIG;

        let mut cmd = [0_u8; 16];
        if cdb.len() > 16 {
            return Err(Error::InvalidCDB);
        }
        cmd[..cdb.len()].copy_from_slice(cdb);
        sgbuf.cmd_len = cdb.len() as u8;
        sgbuf.cmdp = cmd.as_mut_ptr();

        let mut sense_buffer = [0_u8; 64];
        sgbuf.sbp = sense_buffer.as_mut_ptr();
        sgbuf.mx_sb_len = sense_buffer.len() as u8;

        let mut data = match xfer {
            TransferBuffer::None => Vec::new(),
            TransferBuffer::ToDevice(bfr) => Vec::from(bfr),
            TransferBuffer::FromDevice(ref bfr) => vec![0_u8; bfr.len()],
        };

        sgbuf.dxferp = data.as_mut_ptr() as *mut c_void;
        sgbuf.dxfer_len = data.len() as u32;
        sgbuf.dxfer_direction = match xfer {
            TransferBuffer::None => sg::SG_DXFER_NONE,
            TransferBuffer::ToDevice(_) => sg::SG_DXFER_TO_DEV,
            TransferBuffer::FromDevice(_) => sg::SG_DXFER_FROM_DEV,
        };

        debug!("cdb: {:?}", cmd);
        debug!("ioctl_sg_io (before): {:?}", sgbuf);

        // SAFETY:
        // * sg::SG_IO is the correct ioctl number generated by bindgen
        // * sgbuf is a valid pointer to a sg_io_hdr struct generated by bindgen
        // * cmdp is a valid pointer to a 16-byte array and cmd_len is guaranteed to be <= 16
        // * dxferep is a valid pointer to data and dxfer_len is set to its length; data is always mutable
        // * sbp is a valid pointer to sense_buffer and mx_sb_len is set to its length
        unsafe { convert_ioctl_res!(ioctl(self.fd.as_raw_fd(), sg::SG_IO as u64, &mut sgbuf)) }
            .map_err(|e| Error::Nix(e))?;

        debug!("ioctl_sg_io (after ): {:?}", sgbuf);
        debug!("sense: {:?}", sense_buffer);

        if sgbuf.info & sg::SG_INFO_OK_MASK != sg::SG_INFO_OK {
            error!(
                "SG_IO ioctl sgbuf.info is not OK: status: {}, masked_status: {}, host_status: {}",
                sgbuf.status, sgbuf.masked_status, sgbuf.host_status
            );

            if sgbuf.sb_len_wr > 0 {
                error!(
                    "SG_IO sense data: {:?}",
                    sense_buffer.get(..sgbuf.sb_len_wr as usize)
                );
            }

            return Err(Error::SgIoError);
        }

        if let TransferBuffer::FromDevice(bfr) = xfer {
            bfr.copy_from_slice(&data[..bfr.len()]);
        }

        Ok(())
    }
}

impl Backend for Device {
    fn model(&self) -> Model {
        self.info.model
    }

    fn transfer(&mut self, cdb: &[u8]) -> Result<(), Error> {
        self.ioctl_sg_io(cdb, TransferBuffer::None)
    }

    fn transfer_to_device(&mut self, cdb: &[u8], data: &[u8]) -> Result<(), Error> {
        self.ioctl_sg_io(cdb, TransferBuffer::ToDevice(data))
    }

    fn transfer_from_device(&mut self, cdb: &[u8], data: &mut [u8]) -> Result<(), Error> {
        self.ioctl_sg_io(cdb, TransferBuffer::FromDevice(data))
    }
}
