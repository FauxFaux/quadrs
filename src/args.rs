use std::collections::HashMap;
use std::iter::Peekable;

use regex::Regex;

use errors::*;

use usize_from;
use u64_from;
use FileFormat;

pub enum Command {
    From {
        sample_rate: u64,
        format: ::FileFormat,
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
    Write {
        overwrite: bool,
        prefix: String,
    },
    Gen {
        sample_rate: u64,
        cos: Vec<u64>,
    },
    Ui,
}

pub fn parse<'a, I: Iterator<Item = &'a String>>(args: I) -> Result<Vec<Command>> {
    let mut matched = vec![];
    let mut args = args.peekable();

    while let Some(cmd) = args.next() {
        let map = read_just_args(cmd.as_str(), &mut args)?;

        matched.push(match cmd.as_str() {
            "from" => parse_from(&mut args, no_duplicates(map)?)?,
            "shift" => parse_shift(&mut args, no_duplicates(map)?)?,
            "lowpass" => parse_lowpass(&mut args, no_duplicates(map)?)?,
            "sparkfft" => parse_sparkfft(&mut args, no_duplicates(map)?)?,
            "write" => parse_write(&mut args, no_duplicates(map)?)?,
            "gen" => parse_gen(&mut args, map)?,
            "ui" => Command::Ui,
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

    let sample_rate = parse_si_u64(
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
                let ext_start = 1
                    + filename.rfind('.').ok_or_else(|| {
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
        frequency: parse_si_i64(args.next().ok_or("'shift' requires a frequency argument")?)?,
    })
}

fn parse_lowpass<'a, I: Iterator<Item = &'a String>>(
    mut args: I,
    mut map: HashMap<String, String>,
) -> Result<Command> {
    let frequency: u64 = parse_si_u64(
        args.next()
            .ok_or("'lowpass' requires a frequency argument")?
            .as_str(),
    )?;

    // TODO: much better defaults
    let size = match map.remove("power") {
        Some(val) => usize_from(parse_si_u64(&val)?)
            .checked_mul(2)
            .ok_or("power is too large")?,
        None => 40,
    };

    let decimate = match map.remove("decimate") {
        Some(val) => parse_si_u64(&val)?,
        None => 8,
    };

    ensure!(
        map.is_empty(),
        "invalid flags for 'lowpass': {:?}",
        map.keys()
    );

    Ok(Command::LowPass {
        size,
        decimate,
        frequency,
    })
}

fn parse_sparkfft<'a, I: Iterator<Item = &'a String>>(
    args: I,
    mut map: HashMap<String, String>,
) -> Result<Command> {
    let width = match map.remove("width") {
        Some(val) => usize_from(parse_si_u64(&val)?),
        None => 128,
    };

    let stride = match map.remove("stride") {
        Some(val) => parse_si_u64(&val)?,
        None => u64_from(width),
    };

    let (min, max) = match map.remove("range") {
        Some(val) => {
            let (min, max) = val.split_at(val.find(':')
                .ok_or_else(|| format!("range argument must contain a ':': '{}'", val))?);

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

fn parse_write<'a, I: Iterator<Item = &'a String>>(
    mut args: I,
    mut map: HashMap<String, String>,
) -> Result<Command> {
    let overwrite = match map.remove("overwrite") {
        Some(val) => parse_bool(&val)?,
        None => false,
    };

    ensure!(
        map.is_empty(),
        "invalid flags for 'write': {:?}",
        map.keys()
    );

    let prefix: String = args.next()
        .ok_or("'lowpass' requires a frequency argument")?
        .to_string();

    Ok(Command::Write { overwrite, prefix })
}

fn parse_gen<'a, I: Iterator<Item = &'a String>>(
    mut args: I,
    mut map: HashMap<String, Vec<String>>,
) -> Result<Command> {
    let cos: Vec<u64> = match map.remove("cos") {
        Some(val) => val.into_iter()
            .map(|freq| parse_si_u64(&freq))
            .collect::<Result<Vec<u64>>>()?,
        None => bail!("gen requires at least one operation"),
    };

    ensure!(map.is_empty(), "invalid flags for 'gen': {:?}", map.keys());

    let sample_rate = parse_si_u64(args.next().ok_or("'gen' requires a sample rate argument")?)?;

    Ok(Command::Gen { sample_rate, cos })
}

fn guess_sample_rate(filename: &str) -> Result<String> {
    Ok(Regex::new(r"\bsr([0-9]+[kMG]?)\b")?
        .find(filename)
        .ok_or_else(|| {
            format!(
                "can't guess sample rate from '{}', please provide it",
                filename
            )
        })?
        .as_str()[2..]
        .to_string())
}

fn find_multiplication_suffix(from: &str) -> (&str, u32) {
    let (pos, suffix) = match from.char_indices().last() {
        Some(x) => x,
        None => return (from, 1),
    };

    let mul: Option<u32> = match suffix {
        'k' => Some(1_000),
        'M' => Some(1_000_000),
        'G' => Some(1_000_000_000),
        _ => None,
    };

    match mul {
        Some(mul) => (&from[..pos], mul),
        None => (from, 1),
    }
}

fn parse_si_i64(from: &str) -> Result<i64> {
    let (val, mul) = find_multiplication_suffix(from);
    let parsed: i64 = val.parse()?;
    Ok(parsed
        .checked_mul(i64::from(mul))
        .ok_or_else(|| format!("unit is out of range: {}", from))?)
}

fn parse_si_u64(from: &str) -> Result<u64> {
    let (val, mul) = find_multiplication_suffix(from);
    let parsed: u64 = val.parse()?;
    Ok(parsed
        .checked_mul(u64::from(mul))
        .ok_or_else(|| format!("unit is out of range: {}", from))?)
}

fn parse_si_f64(from: &str) -> Result<f64> {
    let (val, mul) = find_multiplication_suffix(from);
    let parsed: f64 = val.parse()?;
    Ok(parsed * f64::from(mul))
}

fn parse_bool(from: &str) -> Result<bool> {
    match from.parse() {
        Ok(val) => Ok(val),
        Err(_) => match from {
            "yes" | "y" => Ok(true),
            "no" | "n" => Ok(false),
            other => bail!("unacceptable boolean value: '{}'", other),
        },
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

fn read_just_args<'a, I>(cmd: &str, iter: &mut Peekable<I>) -> Result<HashMap<String, Vec<String>>>
where
    I: Iterator<Item = &'a String>,
{
    let mut ret = HashMap::new();

    loop {
        // borrow checker :((
        if let Some(opt) = iter.peek() {
            if opt.is_empty() {
                break;
            }

            if !opt.starts_with('-') {
                break;
            }

            // it's a minus, so probably an option.. but is it a number?
            if let Some(c) = opt.chars().nth(2) {
                if c.is_digit(10) {
                    break;
                }
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

        ret.entry(opt[1..].to_string())
            .or_insert_with(|| Vec::new())
            .push(arg.to_string());
    }

    Ok(ret)
}

fn no_duplicates(map: HashMap<String, Vec<String>>) -> Result<HashMap<String, String>> {
    let mut ret = HashMap::with_capacity(map.len());
    for (k, v) in map {
        ensure!(1 == v.len(), "'-{}' specified more than once: {:?}", k, v);
        ret.insert(k, v.into_iter().next().expect("len checked"));
    }
    Ok(ret)
}

#[cfg(test)]
mod tests {
    #[test]
    fn mega() {
        use super::parse_si_u64;
        assert_eq!(123, parse_si_u64("123").unwrap());
        assert_eq!(1_000, parse_si_u64("1k").unwrap());
        assert_eq!(47_000, parse_si_u64("47k").unwrap());
        assert_eq!(0, parse_si_u64("0M").unwrap());
    }
}
