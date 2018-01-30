extern crate byteorder;
#[macro_use]
extern crate conrod;
#[macro_use]
extern crate error_chain;
extern crate image;
extern crate num_complex;
extern crate num_traits;
extern crate palette;
extern crate regex;
extern crate rustfft;

use std::env;
use std::fs;

use std::f64::consts::PI;
const TAU: f64 = PI * 2.;

use byteorder::ByteOrder;

use num_complex::Complex;
use num_traits::identities::Zero;

mod args;
mod errors;
mod fft;
mod filter;
mod gen;
mod samples;
mod shift;
mod ui;

use errors::*;
use samples::Samples;

#[derive(Debug, PartialEq, Eq)]
pub enum FileFormat {
    /// GNU-Radio
    ComplexFloat32,

    /// HackRF
    ComplexInt8,

    /// RTL-SDR
    ComplexUint8,

    /// Fancy
    ComplexInt16,
}

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

    use args::Command::*;
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
            FreqLevels {
                fft_width,
                stride,
                levels,
            } => println!(
                "{:?}",
                fft::freq_levels(
                    samples.as_mut().ok_or("freqlevels requires an input")?,
                    fft_width,
                    stride,
                    levels
                )
            ),
            Write { overwrite, prefix } => do_write(
                samples.as_mut().ok_or("write requires an input")?,
                overwrite,
                &prefix,
            )?,
            Ui => ui::display(samples.as_mut().ok_or("ui requires an input FOR NOW")?)?,
        }
    }

    Ok(())
}

fn do_write(samples: &mut Samples, overwrite: bool, prefix: &str) -> Result<()> {
    if "-" == prefix {
        unimplemented!()
    }

    use std::fs;
    use std::io;
    use byteorder::WriteBytesExt;

    let mut options = fs::OpenOptions::new();
    options.write(true);
    if overwrite {
        options.create(true);
    } else {
        options.create_new(true);
    }

    let filename = format!("{}.sr{}.cf32", prefix, samples.sample_rate());

    let mut file = io::BufWriter::new(options.open(filename)?);
    use byteorder::LittleEndian;

    for off in 0..samples.len() {
        let mut sample = [Complex::zero(); 1];
        // failure: RUST_BACKTRACE=1 cargo run -- from ~/25ms.sr12M.cf32 shift -870k lowpass
        // -decimate 400 1000 write a
        assert_eq!(1, samples.read_at(off, &mut sample));
        file.write_f32::<LittleEndian>(sample[0].re)?;
        file.write_f32::<LittleEndian>(sample[0].im)?;
    }

    Ok(())
}

impl FileFormat {
    fn type_bytes(&self) -> u64 {
        use FileFormat::*;
        match *self {
            ComplexFloat32 => 4,
            ComplexInt8 | ComplexUint8 => 1,
            ComplexInt16 => 2,
        }
    }

    fn pair_bytes(&self) -> u64 {
        self.type_bytes() * 2
    }

    fn to_cf32(&self, buf: &[u8]) -> Complex<f32> {
        assert_eq!(self.pair_bytes(), buf.len() as u64);
        Complex::new(
            self.to_f32(&buf[0..self.type_bytes() as usize]),
            self.to_f32(&buf[self.type_bytes() as usize..2 * self.type_bytes() as usize]),
        )
    }

    fn to_f32(&self, buf: &[u8]) -> f32 {
        use FileFormat::*;
        use byteorder::LittleEndian;

        assert_eq!(self.type_bytes(), buf.len() as u64);

        match *self {
            ComplexFloat32 => LittleEndian::read_f32(buf),

            // TODO: all guesses
            ComplexInt8 => f32::from(buf[0] as i8) / 127.0,
            ComplexUint8 => f32::from(buf[0]) / 255.0 - (255.0 / 2.0),
            ComplexInt16 => f32::from(LittleEndian::read_i16(buf)) / 65535.0 - (65535.0 / 2.0),
        }
    }
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
