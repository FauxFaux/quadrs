use std::collections::HashMap;
use std::collections::hash_map::Entry;

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
    SparkFft {
        width: u32,
        stride: u64,
        min: Option<f32>,
        max: Option<f32>,
    },
}

const COMMANDS: &[&str] = &["from", "shift", "lowpass", "sparkfft"];

pub fn parse<'a, I: Iterator<Item = &'a String>>(args: I) -> Result<Vec<Command>> {
    let mut matched = vec![];
    let mut args = args.peekable();

    while let Some(cmd) = args.next() {

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
                let filename = opts.pop().ok_or("'from' requires a filename argument")?;
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
                    filename: filename.to_string(),
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

                // TODO: much better defaults
                let band = match map.remove("band") {
                    Some(val) => val.parse()?,
                    None => 0.1,
                };

                let decimate = match map.remove("decimate") {
                    Some(val) => val.parse()?,
                    None => 8,
                };

                ensure!(
                    map.is_empty(),
                    "invalid flags for 'lowpass': {:?}",
                    map.keys()
                );

                matched.push(Command::LowPass {
                    band,
                    decimate,
                    frequency,
                })
            }
            "sparkfft" => {
                let mut map = into_map(cmd.as_str(), &opts)?;
                let width = match map.remove("width") {
                    Some(val) => val.parse()?,
                    None => 128u32,
                };

                let stride = match map.remove("stride") {
                    Some(val) => val.parse()?,
                    None => width as u64,
                };

                let (min, max) = match map.remove("range") {
                    Some(val) => {
                        let (min, max) = val.split_at(val.find(':').ok_or_else(|| {
                            format!("range argument must contain a ':': '{}'", val)
                        })?);

                        println!("{} {}", min, max);
                        let min: f32 = min.parse()?;
                        let max: f32 = max.chars().skip(1).collect::<String>().parse()?;

                        (Some(min), Some(max))
                    }
                    None => (None, None),
                };

                ensure!(
                    map.is_empty(),
                    "invalid flags for 'sparkfft': {:?}",
                    map.keys()
                );

                matched.push(Command::SparkFft {
                    width,
                    stride,
                    min,
                    max,
                });
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
        "cf32" | "fc32" => ComplexFloat32,
        "cs8" | "sc8" | "c8" => ComplexInt8,
        "cu8" | "su8" => ComplexUint8,
        "cs16" | "sc16" | "c16" => ComplexInt16,

        other => bail!("unrecognised format code '{}'", other),
    })
}

fn into_map(cmd: &str, vec: &[&String]) -> Result<HashMap<String, String>> {
    let mut ret = HashMap::with_capacity(vec.len() / 2);
    let mut iter = vec.iter();

    while let Some(opt) = iter.next() {
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
