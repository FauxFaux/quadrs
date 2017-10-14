extern crate byteorder;
#[macro_use]
extern crate error_chain;
extern crate regex;
extern crate rustfft;

use std::env;
use std::fs;
use std::mem;

use byteorder::ByteOrder;

use rustfft::FFT;
use rustfft::algorithm::Radix4;
use rustfft::num_complex::Complex;
use rustfft::num_traits::identities::Zero;

mod args;
mod errors;
mod filter;
mod samples;
mod shift;

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
    println!(" lowpass [-power 40] [-decimate 8] FREQUENCY \\");
    println!("sparkfft [-width 128] [-stride STRIDE] [-range LOW:HIGH]");

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


    let commands = args::parse(args)?;
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
            Shift { frequency } => {
                let orig = samples.ok_or("shift requires an input")?;
                let sample_rate = orig.sample_rate();
                samples = Some(Box::new(shift::Shift::new(orig, frequency, sample_rate)))
            }
            LowPass {
                power,
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
                    power,
                )))
            }
            SparkFft {
                width,
                stride,
                min,
                max,
            } => {
                spark_fft(
                    samples.as_mut().ok_or("sparkfft requires an input")?,
                    width,
                    stride,
                    min,
                    max,
                )?;
            }
        }
    }

    Ok(())
}

fn spark_fft(
    samples: &mut Samples,
    fft_width: usize,
    stride: u64,
    min: Option<f32>,
    max: Option<f32>,
) -> Result<()> {

    // TODO: super dumb:
    let min = min.unwrap_or(0.08);
    let max = max.unwrap_or(1.);

    let fft_width = fft_width as usize;

    let fft = Radix4::new(fft_width as usize, false);

    let mut i = 0;
    while i < (samples.len() - fft_width as u64) {

        let mut inp = vec![Complex::zero(); fft_width];
        samples.read_exact_at(i, &mut inp)?;

        let mut out = vec![Complex::zero(); fft_width];

        fft.process(&mut inp, &mut out);
        mem::drop(inp); // inp is now junk

        let top = '█';
        let bot = ' ';
        let graph: Vec<char> = "▁▂▃▄▅▆▇".chars().collect();

        #[cfg(never)]
        let max = out.iter()
            .map(|x| x.norm())
            .max_by(|x, y| x.partial_cmp(y).unwrap())
            .unwrap();

        let distinction = (max - min) / (graph.len() as f32);
        let mut buf = String::with_capacity(fft_width);
        for val in out.iter().skip(fft_width / 2).chain(
            out.iter().take(fft_width / 2),
        )
        {
            let norm = val.norm();
            if norm < min {
                buf.push(bot);
            } else if norm >= max {
                buf.push(top);
            } else {
                buf.push(graph[((norm - min) / distinction) as usize]);
            }
        }

        println!("│{}│", buf);

        i += stride;
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
            self.to_f32(
                &buf[self.type_bytes() as usize..2 * self.type_bytes() as usize],
            ),
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
