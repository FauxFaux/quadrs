
extern crate byteorder;
#[macro_use]
extern crate conrod;
#[macro_use]
extern crate error_chain;
extern crate image;
extern crate num_complex;
extern crate num_traits;
extern crate octagon;
extern crate palette;
extern crate regex;
extern crate rustfft;

use std::env;
use std::fs;

use octagon::gen;
use octagon::fft;
use octagon::filter;
use octagon::samples;
use octagon::shift;

use octagon::samples::Samples;

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

    let mut samples: Option<Box<Samples>> = None;

    use octagon::Command::*;
    for cmd in commands {
        match cmd {
            From {
                filename,
                format,
                sample_rate,
            } => {
                samples = Some(Box::new(samples::SampleFile::new(
                    fs::File::open(filename)?,
                    format,
                    sample_rate,
                )))
            }
            Gen {
                sample_rate,
                cos,
                seconds,
            } => samples = Some(Box::new(gen::Gen::new(cos, sample_rate, seconds)?)),
            Shift { frequency } => {
                let orig = samples.ok_or("shift requires an input")?;
                let sample_rate = orig.sample_rate();
                samples = Some(Box::new(shift::Shift::new(orig, frequency, sample_rate)))
            }
            LowPass {
                size,
                decimate,
                frequency,
            } => {
                let orig = samples.ok_or("lowpass requires an input")?;
                let original_sample_rate = orig.sample_rate();
                samples = Some(Box::new(filter::LowPass::new(
                    orig,
                    frequency,
                    decimate,
                    original_sample_rate,
                    size,
                )))
            }
            SparkFft {
                width,
                stride,
                min,
                max,
            } => {
                fft::spark_fft(
                    samples.as_mut().ok_or("sparkfft requires an input")?,
                    width,
                    stride,
                    min,
                    max,
                )?;
            }
            Bucket {
                fft_width,
                stride,
                levels,
            } => println!(
                "{}",
                fft::freq_levels(
                    samples.as_mut().ok_or("bucket -by freq requires an input")?,
                    fft_width,
                    stride,
                    levels
                ).vals
                    .into_iter()
                    .map(|x| format!("{}", x))
                    .collect::<String>()
            ),
            Write { overwrite, prefix } => octagon::do_write(
                samples.as_mut().ok_or("write requires an input")?,
                overwrite,
                &prefix,
            )?,
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
