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

use asm2x6xtool::*;
use env_logger::{Builder, Env};
use std::fs::File;
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    let devices = usb::Devices::enumerate()?;

    for device in devices.into_iter() {
        println!("device: {:?}", device);
        let usbdev = usb::Device::new(device)?;
        let mut device = asm2x6x::Device::new(Box::new(usbdev));

        println!("firmware version: {}", device.read_fw_version()?);

        println!("reading configuration");
        File::create("config.b")?.write_all(&device.read_config()?)?;

        println!("reading firmware");
        let mut fw = [0_u8; 0xff00];
        device.read_firmware(&mut fw)?;
        File::create("firmware.b")?.write_all(&fw)?;
    }

    Ok(())
}
