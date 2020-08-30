#[macro_use]
extern crate conrod;

use std::env;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Error;

mod args;
mod ui;

fn usage(us: &str) {
    println!("usage: {} \\", us);
    println!("    from [-sr SAMPLE_RATE] [-format cf32|cs8|cu8|cs16] FILENAME.sr32k.cf32 \\");
    println!("   shift [-]FREQUENCY \\");
    println!(" lowpass [-power 20] [-decimate 8] FREQUENCY \\");
    println!("sparkfft [-width 128] [-stride =width] [-range LOW:HIGH] \\");
    println!("  bucket [-width 128] [-stride =width] [-by freq] COUNT \\");
    println!("   write [-overwrite no] FILENAME_PREFIX \\");
    println!("     gen [-cos FREQUENCY]* [-len 1 (second)] SAMPLE_RATE \\");

    println!();
    println!();
    println!("Formats:");
    println!();
    println!(" * cf32: complex (little endian) floats, 32-bit (GNU-Radio, gqrx)");
    println!(" *  cs8: complex      signed (integers),  8-bit (HackRF)");
    println!(" *  cu8: complex    unsigned (integers),  8-bit (RTL-SDR)");
    println!(" * cs16: complex      signed (integers), 16-bit (Fancy)");
    println!();
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    let mut args = args.iter();
    let us = args.next().unwrap();

    let commands = match args::parse(args) {
        Ok(commands) => commands,
        Err(e) => {
            usage(us);
            bail!(e);
        }
    };

    if commands.is_empty() {
        usage(us);
        bail!("no commands provided");
    }

    let mut samples = None;
    for command in commands {
        use crate::args::Command::*;
        match command {
            Octagon(op) => samples = op.exec(samples)?,
            Ui => ui::display(
                samples
                    .as_mut()
                    .ok_or_else(|| anyhow!("ui requires an input FOR NOW"))?,
            )?,
        }
    }

    Ok(())
}
