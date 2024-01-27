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

use crate::asm2x6x::Info;
use asm2x6xtool::*;
use clap::{Parser, Subcommand};
use env_logger::{Builder, Env};
use log::info;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Subcommand)]
enum Commands {
    /// read firmware from device to file
    ReadFirmware {
        /// file to write firmware to
        output: PathBuf,
    },

    /// read configuration from device to file
    ReadConfiguration {
        /// file to write configuration to
        output: PathBuf,
    },

    /// list all connected devices
    ListDevices,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Optional device name to operate on
    #[arg(short, long)]
    device: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

pub fn find_devices() -> Result<Vec<Box<dyn Info>>, crate::error::Error> {
    let mut devices = Vec::<Box<dyn Info>>::new();

    #[cfg(target_os = "linux")]
    crate::linux::find_devices(&mut devices)?;

    crate::usb::find_devices(&mut devices)?;

    Ok(devices)
}

fn find_device(name: Option<String>) -> Result<asm2x6x::Device, Box<dyn std::error::Error>> {
    let devices = find_devices()?;

    if devices.is_empty() {
        return Err("no devices found".into());
    }

    match name {
        None => {
            return Ok(asm2x6x::Device::new(
                devices
                    .into_iter()
                    .next()
                    .expect(
                        "devices.is_empty() was false but devices.into_iter().next() returned None",
                    )
                    .open()?,
            ));
        }
        Some(name) => {
            for device in devices.into_iter() {
                if device.to_string() == name {
                    return Ok(asm2x6x::Device::new(device.open()?));
                }
            }
        }
    }

    return Err("device not found".into());
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::ReadFirmware { output } => {
            let mut device = find_device(cli.device)?;

            info!("reading firmware");
            File::create(output)?.write_all(&device.read_firmware()?)?;
        }

        Commands::ReadConfiguration { output } => {
            let mut device = find_device(cli.device)?;

            info!("reading configuration");
            File::create(output)?.write_all(&device.read_config()?)?;
        }

        Commands::ListDevices => {
            let devices = find_devices()?;

            if devices.is_empty() {
                info!("no devices found");
            }

            for device in devices.into_iter() {
                info!("{} - {}", device.to_string(), device.model());
            }
        }
    }

    Ok(())
}
