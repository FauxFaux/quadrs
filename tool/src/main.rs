#[macro_use]
extern crate conrod;
#[macro_use]
extern crate error_chain;
extern crate image;
extern crate octagon;
extern crate palette;
extern crate regex;
extern crate rustfft;

use std::env;

mod args;
mod errors;
mod ui;

use errors::*;

quick_main!(run);

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

fn run() -> Result<()> {
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
        use args::Command::*;
        match command {
            Octagon(op) => samples = op.exec(samples)?,
            Ui => ui::display(samples.as_mut().ok_or("ui requires an input FOR NOW")?)?,
        }
    }

    Ok(())
}

// clippy
#[allow(unknown_lints, absurd_extreme_comparisons)]
fn usize_from(val: u64) -> usize {
    assert!(val <= std::usize::MAX as u64);
    val as usize
}

// clippy
#[allow(unknown_lints, absurd_extreme_comparisons)]
fn u64_from(val: usize) -> u64 {
    assert!((val as u64) <= std::u64::MAX);
    val as u64
}
