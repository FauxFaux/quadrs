extern crate byteorder;
#[macro_use]
extern crate error_chain;
extern crate regex;
extern crate rustfft;

use std::env;
use std::fs;
use std::mem;
use std::path;

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
    println!(" lowpass [-band BAND] [-decimate DECIMATE] FREQUENCY \\");
    println!("sparkfft [-width 128] [-stride STRIDE]");

    println!();
    println!();
    println!("Formats:");
    println!();
    println!(" * cf32: complex (little endian) floats, 32-bit (GNU-Radio, gqrx)");
    println!(" *  cs8: complex      signed (integers),  8-bit (HackRF)");
    println!(" *  cu8: complex    unsigned (integers),  8-bit (RTL-SDR)");
    println!(" * cs16: complex      signed (integers), 16-bit (Fancy)");
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut args = args.iter();
    let us = args.next().unwrap();


    let mut commands = args::parse(args)?;
    if commands.is_empty() {
        usage(us);
        bail!("no commands provided");
    }

    let from = commands.remove(0);

    let (path, format, sample_rate) = if let args::Command::From {
        filename,
        format,
        sample_rate,
    } = from
    {
        (path::PathBuf::from(filename), format, sample_rate)
    } else {
        bail!("first command must be 'from'");
    };

    let mut samples: Box<Samples> =
        Box::new(samples::SampleFile::new(fs::File::open(path)?, format));

    use args::Command::*;
    for cmd in commands {
        match cmd {
            From { .. } => bail!("multiple from commands are unsupported"),
            Shift { frequency } => {
                samples = Box::new(shift::Shift::new(samples, frequency, sample_rate));
            }
            LowPass {
                band,
                decimate,
                frequency,
            } => {
                samples = Box::new(filter::LowPass::new(
                    samples,
                    frequency,
                    decimate,
                    sample_rate,
                    band,
                ))
            }
            SparkFft { width, stride } => {
                spark_fft(&mut samples, width, stride)?;
            }
        }
    }

    Ok(())
}

fn spark_fft(samples: &mut Samples, fft_width: u32, stride: u64) -> Result<()> {

    let fft_width = fft_width as usize;

    let fft = Radix4::new(fft_width as usize, false);

    let mut i = 0;
    while i < (samples.len() - fft_width as u64) {

        let mut inp = vec![Complex::zero(); fft_width];
        samples.read_exact_at(i, &mut inp)?;

        let mut out = vec![Complex::zero(); fft_width];

        fft.process(&mut inp, &mut out);
        mem::drop(inp); // inp is now junk

        let graph: Vec<char> = " ▁▂▃▄▅▆▇█".chars().collect();

        let max = out.iter()
            .map(|x| x.norm())
            .max_by(|x, y| x.partial_cmp(y).unwrap())
            .unwrap();
        let distinction = (max + 1.) / (graph.len() as f32);
        let mut buf = String::with_capacity(fft_width);
        for val in out.iter().skip(fft_width / 2).chain(
            out.iter().take(fft_width / 2),
        )
        {
            let norm = val.norm();
            buf.push(graph[(norm / distinction) as usize]);
        }

        println!("{}", buf);

        i += fft_width as u64;
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
