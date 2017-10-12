use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::Path;

use regex::Regex;

use errors::*;

use FileFormat;

pub enum Command {
    From {
        sample_rate: u64,
        format: ::FileFormat,
        filename: String,
    },
    Shift { frequency: i64 },
    LowPass {
        band: f32,
        decimate: u64,
        frequency: u64,
    },
    SparkFft { width: u32, stride: u64 },
}

const COMMANDS: &[&str] = &["from", "shift", "lowpass", "sparkfft"];

pub fn parse<I: Iterator<Item = String>>(args: I) -> Result<Vec<Command>> {
    let mut matched = vec![];
    let mut args = args.peekable();
    let us = args.next().expect("argv[0] must be present");

    loop {
        let cmd = match args.next() {
            Some(cmd) => cmd,
            None => break,
        };

        let mut opts = vec![];
        loop {
            match args.peek() {
                Some(val) if !COMMANDS.contains(&val.as_str()) => val,
                _ => break,
            };
            opts.push(args.next().expect("just peeked that"));
        }

        match cmd.as_str() {
            "from" => {
                let filename: String = opts.pop().ok_or("'from' requires a filename argument")?;
                let mut map = into_map(cmd.as_str(), &opts)?;
                let provided_sample_rate = map.remove("sr");
                let provided_format = map.remove("format");
                ensure!(map.is_empty(), "invalid flags for 'from': {:?}", map.keys());

                let sample_rate = parse_si(
                    match provided_sample_rate {
                        Some(rate) => rate,
                        None => {
                            Regex::new(r"\bsr([0-9]+[kMG]?)\b")?
                                .find(filename.as_str())
                                .ok_or_else(|| {
                                    format!(
                                        "can't guess sample rate from '{}', please provide it",
                                        filename
                                    )
                                })?
                                .as_str()
                                [2..]
                                .to_string()
                        }
                    }.as_str(),
                )?;

                let format = guess_from_extension(
                    match provided_format {
                        Some(fmt) => fmt.to_string(),
                        None => {
                            // EURGH
                            let ext_start = 1 +
                                filename.rfind('.').ok_or_else(|| {
                                    format!("can't guess format as no extension: '{}'", filename)
                                })?;
                            String::from_utf8(filename.bytes().skip(ext_start).collect()).unwrap()
                        }
                    }.as_str(),
                )?;

                matched.push(Command::From {
                    sample_rate,
                    format,
                    filename,
                });

            }
            "shift" => {
                ensure!(opts.len() == 1, "'shift' has only one argument: frequency");
                matched.push(Command::Shift {
                    frequency: opts.pop()
                        .ok_or("'shift' requires a frequency argument")?
                        .parse()?,
                });
            }
            "lowpass" => {
                let frequency: u64 = parse_si(
                    opts.pop()
                        .ok_or("'lowpass' requires a frequency argument")?
                        .as_str(),
                )?;
                let mut map = into_map(cmd.as_str(), &opts)?;
                let provided_band = map.remove("band");
                let provided_decimate = map.remove("decimate");
                ensure!(
                    map.is_empty(),
                    "invalid flags for 'lowpass': {:?}",
                    map.keys()
                );

                // TODO: much better defaults
                matched.push(Command::LowPass {
                    band: provided_band.unwrap_or("0.1".to_string()).parse()?,
                    decimate: provided_decimate.unwrap_or("8".to_string()).parse()?,
                    frequency,
                })
            }
            "sparkfft" => {
                let mut map = into_map(cmd.as_str(), &opts)?;
                let width = map.remove("width").unwrap_or("128".to_string()).parse()?;
                let stride = map.remove("stride").unwrap_or("1".to_string()).parse()?;
                ensure!(
                    map.is_empty(),
                    "invalid flags for 'sparkfft': {:?}",
                    map.keys()
                );

                matched.push(Command::SparkFft { width, stride });
            }
            other => bail!("unrecognised command: '{}'", other),
        }
    }

    Ok(matched)
}

fn parse_si(from: &str) -> Result<u64> {
    let last = from.chars().last().ok_or(
        "empty strings aren't valid integers",
    )?;
    let mul: Option<u64> = match last {
        'k' => Some(1_000),
        'M' => Some(1_000_000),
        'G' => Some(1_000_000_000),
        _ => None,
    };

    if let Some(mul) = mul {
        let prefix: String = from.chars().take(from.chars().count() - 1).collect();
        let parsed: u64 = prefix.parse()?;
        Ok(parsed.checked_mul(mul).ok_or_else(|| {
            format!("unit is out of range: {}", from)
        })?)
    } else {
        Ok(from.parse()?)
    }
}

fn guess_from_extension(ext: &str) -> Result<FileFormat> {
    use FileFormat::*;
    Ok(match ext {
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

        other => bail!("unrecognised format code '{}'", other),
    })
}

fn into_map(cmd: &str, vec: &[String]) -> Result<HashMap<String, String>> {
    let mut ret = HashMap::with_capacity(vec.len() / 2);
    let mut iter = vec.iter();
    loop {
        let opt = match iter.next() {
            Some(opt) => opt,
            None => break,
        };

        if opt.is_empty() || !opt.starts_with('-') {
            bail!(
                "{} encountered an argument that doesn't look like an argument: '{}'",
                cmd,
                opt
            );
        }

        let arg = match iter.next() {
            Some(arg) if arg.is_empty() => {
                bail!("{} .. {} requires a non-empty argument", cmd, opt)
            }
            Some(arg) => arg,
            None => bail!("{} .. {} requires an argument", cmd, opt),
        };

        match ret.entry(opt[1..].to_string()) {
            Entry::Vacant(vacant) => {
                vacant.insert(arg.to_string());
            }
            Entry::Occupied(entry) => {
                bail!(
                    "{} .. {} specified twice, once with '{}' and once with '{}'",
                    cmd,
                    opt,
                    arg,
                    entry.get()
                )
            }
        }
    }

    Ok(ret)
}
