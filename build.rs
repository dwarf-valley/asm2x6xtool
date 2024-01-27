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

#[cfg(target_os = "linux")]
fn gen_sg_bindings() {
    use std::env;
    use std::path::PathBuf;
    extern crate bindgen;

    let bindings = bindgen::Builder::default()
        .header_contents("sg.h", "#include <scsi/sg.h>")
        .derive_debug(true)
        .derive_default(true)
        .generate()
        .expect("Unable to generate scsi/sg.h bindings");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("sg.rs"))
        .expect("Couldn't write bindings!");
}

fn main() {
    #[cfg(target_os = "linux")]
    gen_sg_bindings();
}
