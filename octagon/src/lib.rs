extern crate byteorder;
#[macro_use]
extern crate failure;
extern crate num_complex;
extern crate num_traits;
extern crate rustfft;

pub mod bits;
mod fft;
mod filter;
mod gen;
mod samples;
mod shift;

use std::f64::consts::PI;
use std::fs;

use byteorder::ByteOrder;
use failure::Error;
use num_complex::Complex;
use num_traits::identities::Zero;

pub use crate::samples::Samples;

const TAU: f64 = PI * 2.;

#[derive(Debug, Clone)]
pub enum Operation {
    From {
        sample_rate: u64,
        format: crate::FileFormat,
        filename: String,
    },
    Shift {
        frequency: i64,
    },
    LowPass {
        size: usize,
        decimate: u64,
        frequency: u64,
    },
    SparkFft {
        width: usize,
        stride: u64,
        min: Option<f32>,
        max: Option<f32>,
    },
    Bucket {
        fft_width: usize,
        stride: u64,
        levels: usize,
    },
    Write {
        overwrite: bool,
        prefix: String,
    },
    Gen {
        seconds: f64,
        sample_rate: u64,
        cos: Vec<i64>,
    },
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
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

impl Operation {
    pub fn exec(
        &self,
        mut samples: Option<Box<dyn Samples>>,
    ) -> Result<Option<Box<dyn Samples>>, Error> {
        use crate::Operation::*;
        Ok(match *self {
            From {
                ref filename,
                format,
                sample_rate,
            } => Some(Box::new(samples::SampleFile::new(
                fs::File::open(filename)?,
                format,
                sample_rate,
            ))),
            Gen {
                sample_rate,
                ref cos,
                seconds,
            } => Some(Box::new(gen::Gen::new(cos.to_vec(), sample_rate, seconds)?)),
            Shift { frequency } => {
                let orig = samples.ok_or_else(|| format_err!("shift requires an input"))?;
                let sample_rate = orig.sample_rate();
                Some(Box::new(shift::Shift::new(orig, frequency, sample_rate)))
            }
            LowPass {
                size,
                decimate,
                frequency,
            } => {
                let orig = samples.ok_or_else(|| format_err!("lowpass requires an input"))?;
                let original_sample_rate = orig.sample_rate();
                Some(Box::new(filter::LowPass::new(
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
                    samples
                        .as_mut()
                        .ok_or_else(|| format_err!("sparkfft requires an input"))?,
                    width,
                    stride,
                    min,
                    max,
                )?;
                samples
            }
            Bucket {
                fft_width,
                stride,
                levels,
            } => {
                println!(
                    "{}",
                    fft::freq_levels(
                        samples
                            .as_mut()
                            .ok_or_else(|| format_err!("bucket -by freq requires an input"))?,
                        fft_width,
                        stride,
                        levels
                    )
                    .vals
                    .into_iter()
                    .map(|x| format!("{}", x))
                    .collect::<String>()
                );
                samples
            }
            Write {
                overwrite,
                ref prefix,
            } => {
                do_write(
                    samples
                        .as_mut()
                        .ok_or_else(|| format_err!("write requires an input"))?,
                    overwrite,
                    prefix,
                )?;
                samples
            }
        })
    }
}

fn do_write(samples: &mut dyn Samples, overwrite: bool, prefix: &str) -> Result<(), Error> {
    if "-" == prefix {
        unimplemented!()
    }

    use byteorder::WriteBytesExt;
    use std::io;

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

    let mut off = 0;
    while off < samples.len() {
        let mut buf = [Complex::zero(); 0x1000];
        let read = samples.read_at(off, &mut buf);
        assert_ne!(0, read, "short read at offset {} of {}", off, samples.len());
        off += read as u64;

        for sample in &buf[..read] {
            file.write_f32::<LittleEndian>(sample.re)?;
            file.write_f32::<LittleEndian>(sample.im)?;
        }
    }

    Ok(())
}

impl FileFormat {
    fn type_bytes(&self) -> u64 {
        use crate::FileFormat::*;
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
        use crate::FileFormat::*;
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
