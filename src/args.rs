use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::iter::Peekable;

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

pub fn parse<'a, I: Iterator<Item = &'a String>>(args: I) -> Result<Vec<Command>> {
    let mut matched = vec![];
    let mut args = args.peekable();

    while let Some(cmd) = args.next() {
        let map = read_just_args(cmd.as_str(), &mut args)?;

        matched.push(match cmd.as_str() {
            "from" => parse_from(&mut args, map)?,
            "shift" => parse_shift(&mut args, map)?,
            "lowpass" => parse_lowpass(&mut args, map)?,
            "sparkfft" => parse_sparkfft(&mut args, map)?,
            other => bail!("unrecognised command: '{}'", other),
        });
    }

    Ok(matched)
}

fn parse_from<'a, I: Iterator<Item = &'a String>>(
    mut args: I,
    mut map: HashMap<String, String>,
) -> Result<Command> {
    let filename = args.next().ok_or("'from' requires a filename argument")?;
    let provided_sample_rate = map.remove("sr");
    let provided_format = map.remove("format");
    ensure!(map.is_empty(), "invalid flags for 'from': {:?}", map.keys());
    let sample_rate = parse_si(
        match provided_sample_rate {
            Some(rate) => rate,
            None => guess_sample_rate(filename)?,
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

    Ok(Command::From {
        sample_rate,
        format,
        filename: filename.to_string(),
    })
}

fn parse_shift<'a, I: Iterator<Item = &'a String>>(
    mut args: I,
    map: HashMap<String, String>,
) -> Result<Command> {
    ensure!(map.is_empty(), "'shift' has no named arguments");
    Ok(Command::Shift {
        frequency: args.next()
            .ok_or("'shift' requires a frequency argument")?
            .parse()?,
    })
}

fn parse_lowpass<'a, I: Iterator<Item = &'a String>>(
    mut args: I,
    mut map: HashMap<String, String>,
) -> Result<Command> {
    let frequency: u64 = parse_si(
        args.next()
            .ok_or("'lowpass' requires a frequency argument")?
            .as_str(),
    )?;

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

    Ok(Command::LowPass {
        band,
        decimate,
        frequency,
    })
}

fn parse_sparkfft<'a, I: Iterator<Item = &'a String>>(
    args: I,
    mut map: HashMap<String, String>,
) -> Result<Command> {
    let width = match map.remove("width") {
        Some(val) => val.parse()?,
        None => 128u32,
    };

    let stride = match map.remove("stride") {
        Some(val) => val.parse()?,
        None => u64::from(width),
    };

    let (min, max) = match map.remove("range") {
        Some(val) => {
            let (min, max) = val.split_at(val.find(':').ok_or_else(|| {
                format!("range argument must contain a ':': '{}'", val)
            })?);

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

    Ok(Command::SparkFft {
        width,
        stride,
        min,
        max,
    })
}

fn guess_sample_rate(filename: &str) -> Result<String> {
    Ok(
        Regex::new(r"\bsr([0-9]+[kMG]?)\b")?
            .find(filename)
            .ok_or_else(|| {
                format!(
                    "can't guess sample rate from '{}', please provide it",
                    filename
                )
            })?
            .as_str()
            [2..]
            .to_string(),
    )
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

fn read_just_args<'a, I>(cmd: &str, iter: &mut Peekable<I>) -> Result<HashMap<String, String>>
where
    I: Iterator<Item = &'a String>,
{
    let mut ret = HashMap::new();

    loop {
        // borrow checker :((
        if let Some(opt) = iter.peek() {
            if opt.is_empty() || !opt.starts_with('-') {
                break;
            }
        } else {
            break;
        }

        let opt = iter.next().expect("just peeked that");

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
