extern crate byteorder;
#[macro_use]
extern crate error_chain;
extern crate rustfft;

use std::env;
use std::ffi;
use std::fs;
use std::io::Read;
use std::mem;
use std::path;

use byteorder::ByteOrder;

use rustfft::FFT;
use rustfft::algorithm::Radix4;
use rustfft::num_complex::Complex;
use rustfft::num_traits::identities::Zero;

mod errors;

use errors::*;

#[derive(Debug, PartialEq, Eq)]
enum FileFormat {
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

fn run() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut args = args.iter();
    let us = args.next().unwrap();

    let (path, format) = match args.next().map(|x| x.as_str()) {
        Some("-from") => {
            guess_input(args.next()).chain_err(
                || "guessing -from argument",
            )?
        }
        Some("-from-cf32") => (path_or_bail(args.next())?, FileFormat::ComplexFloat32),
        // TODO: others,
        other => bail!("unrecognised from: {:?}", other),
    };

    let mut file = fs::File::open(path)?;

    const FFT_WIDTH: usize = 256;

    let fft = Radix4::new(FFT_WIDTH, false);

    loop {
        let mut buf = vec![0u8; FFT_WIDTH * 2 * format.type_bytes()];
        file.read_exact(&mut buf)?;
        let mut inp: Vec<Complex<f32>> = buf.chunks(2 * format.type_bytes())
            .map(|two_vals| format.to_cf32(two_vals))
            .collect();

        let mut out = vec![Complex::zero(); FFT_WIDTH];

        fft.process(&mut inp, &mut out);
        mem::drop(inp); // inp is now junk

        let graph: Vec<char> = " ▁▂▃▄▅▆▇█".chars().collect();

        let distinction = 1.0 / (graph.len() as f32);

        let mut buf = String::with_capacity(FFT_WIDTH);
        for val in out.iter().skip(FFT_WIDTH / 2).chain(out.iter().take(FFT_WIDTH / 2)) {
            let norm = val.norm();
            if norm > 1.0 {
                buf.push(graph[graph.len() - 1]);
            } else {
                buf.push(graph[(norm / distinction) as usize]);
            }
        }

        println!("{}", buf);
    }

    Ok(())
}

impl FileFormat {
    fn type_bytes(&self) -> usize {
        use FileFormat::*;
        match *self {
            ComplexFloat32 => 4,
            ComplexInt8 => 1,
            ComplexUint8 => 1,
            ComplexInt16 => 2,
        }
    }

    fn to_cf32(&self, buf: &[u8]) -> Complex<f32> {
        assert_eq!(self.type_bytes() * 2, buf.len());
        Complex::new(
            self.to_f32(&buf[0..self.type_bytes()]),
            self.to_f32(&buf[self.type_bytes()..self.type_bytes() * 2]),
        )
    }

    fn to_f32(&self, buf: &[u8]) -> f32 {
        use FileFormat::*;
        use byteorder::LittleEndian;

        assert_eq!(self.type_bytes(), buf.len());

        match *self {
            ComplexFloat32 => LittleEndian::read_f32(buf),

            // TODO: all guesses
            ComplexInt8 => f32::from(buf[0] as i8) / 127.0,
            ComplexUint8 => f32::from(buf[0]) / 255.0 - (255.0 / 2.0),
            ComplexInt16 => f32::from(LittleEndian::read_i16(buf)) / 65535.0 - (65535.0 / 2.0),
        }
    }
}

fn path_or_bail(arg: Option<&String>) -> Result<path::PathBuf> {
    arg.map(|path| path.into()).ok_or(
        "path argument required, but not provided"
            .into(),
    )
}

fn guess_input(arg: Option<&String>) -> Result<(path::PathBuf, FileFormat)> {
    let path = path_or_bail(arg)?;
    let fmt = guess_from_extension(match path.extension() {
        Some(ext) => ext,
        None => {
            bail!(
                "can't guess filetype from '{:?}' as it has no extension; use a -from-* variant",
                path
            )
        }
    })?;

    Ok((path, fmt))
}

fn guess_from_extension(ext: &ffi::OsStr) -> Result<FileFormat> {
    use FileFormat::*;
    Ok(match ext.to_string_lossy().as_ref() {
        "cf32" => ComplexFloat32,
        "cs8" => ComplexInt8,
        "cu8" => ComplexUint8,
        "cs16" => ComplexInt16,

        // Non-canonical extensions
        "fc32" => ComplexFloat32,
        "sc8" => ComplexInt8,
        "c8" => ComplexInt8,
        "su8" => ComplexUint8,
        "sc16" => ComplexInt16,
        "c16" => ComplexInt16,

        other => {
            bail!(
                "can't guess filetype from unrecognised extension '{}', use a -from-* variant",
                other
            )
        }
    })
}
