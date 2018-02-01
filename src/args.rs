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
    Ui,
}

pub fn parse<'a, I: Iterator<Item = &'a String>>(args: I) -> Result<Vec<Command>> {
    let mut matched = vec![];
    let mut args = args.peekable();

    while let Some(cmd) = args.next() {
        let map = read_just_args(&mut args).chain_err(|| format!("finding args for {:?}", cmd))?;

        matched.push(match cmd.as_str() {
            "from" => parse_from(&mut args, no_duplicates(map)?),
            "shift" => parse_shift(&mut args, no_duplicates(map)?),
            "lowpass" => parse_lowpass(&mut args, no_duplicates(map)?),
            "sparkfft" => parse_sparkfft(&mut args, no_duplicates(map)?),
            "bucket" => parse_bucket(&mut args, no_duplicates(map)?),
            "write" => parse_write(&mut args, no_duplicates(map)?),
            "gen" => parse_gen(&mut args, map),
            "ui" => parse_ui(&mut args, no_duplicates(map)?),
            _ => Err("unrecognised command".into()),
        }.chain_err(|| format!("processing command: {:?}", cmd))?);
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
    ensure!(map.is_empty(), "invalid flags: {:?}", map.keys());

    let mut sample_rate = None;

    if let Some(guess) = guess_sample_rate(filename) {
        sample_rate = Some(guess);
    }

    let mut format = None;

    // More specifically, it could be a gqrx file of this format:
    // gqrx_20180126_111922_868000000_8000000_fc.raw'
    if let Some(gqrx_sample_rate) = Regex::new("gqrx_[0-9]{8}_[0-9]{6}_[0-9]+_([0-9]+)_fc.raw")
        .unwrap()
        .captures_iter(filename)
        .next() {
        sample_rate = Some(gqrx_sample_rate[1].to_string());
        format = Some(FileFormat::ComplexFloat32);
    }

    if let Some(provided) = provided_sample_rate {
        sample_rate = Some(provided);
    }

    let sample_rate = sample_rate;

    if let Some(dot) = filename.rfind('.') {
        let ext_start = 1 + dot;
        let ext = String::from_utf8(filename.bytes().skip(ext_start).collect()).unwrap();
        if let Some(guess) = guess_from_extension(&ext) {
            format = Some(guess);
        }
    }

    if let Some(provided) = provided_format {
        format = Some(guess_from_extension(&provided).ok_or_else(|| format!("unrecognised extension: {:?}", provided))?);
    }

    Ok(Command::From {
        sample_rate: parse_si_u64(&sample_rate.ok_or_else(|| format!("unable to guess format from filename {:?}, please specify it", filename))?)?,
        format: format.ok_or_else(|| format!("unable to guess format from filename {:?}, please specify it", filename))?,
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
    let frequency: u64 = parse_si_u64(args.next()
        .ok_or("'lowpass' requires a frequency argument")?)?;

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

    ensure!(map.is_empty(), "invalid flags: {:?}", map.keys());

    Ok(Command::LowPass {
        size,
        decimate,
        frequency,
    })
}

fn parse_sparkfft<'a, I: Iterator<Item = &'a String>>(
    _args: I,
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

    ensure!(map.is_empty(), "invalid flags: {:?}", map.keys());

    Ok(Command::SparkFft {
        width,
        stride,
        min,
        max,
    })
}

fn parse_bucket<'a, I: Iterator<Item = &'a String>>(
    mut args: I,
    mut map: HashMap<String, String>,
) -> Result<Command> {
    let levels = args.next()
        .ok_or("bucket usage: bucket -by freq [number-of-buckets]")?
        .parse()?;

    let fft_width = match map.remove("width") {
        Some(val) => usize_from(parse_si_u64(&val)?),
        None => 128,
    };

    let stride = match map.remove("stride") {
        Some(val) => parse_si_u64(&val)?,
        None => u64_from(fft_width),
    };

    match map.remove("by") {
        Some(ref s) if s == "freq" => {}
        other => bail!("must bucket -by freq, not {:?}", other),
    }

    ensure!(map.is_empty(), "invalid flags: {:?}", map.keys());

    Ok(Command::Bucket {
        fft_width,
        stride,
        levels,
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

    ensure!(map.is_empty(), "invalid flags: {:?}", map.keys());

    let prefix: String = args.next()
        .ok_or("'lowpass' requires a frequency argument")?
        .to_string();

    Ok(Command::Write { overwrite, prefix })
}

fn parse_gen<'a, I: Iterator<Item = &'a String>>(
    mut args: I,
    mut map: HashMap<String, Vec<String>>,
) -> Result<Command> {
    let cos: Vec<i64> = match map.remove("cos") {
        Some(val) => val.into_iter()
            .map(parse_si_i64)
            .collect::<Result<Vec<i64>>>()
            .chain_err(|| "parsing -cos")?,
        None => bail!("gen requires at least one operation"),
    };

    let seconds = match map.remove("len") {
        Some(ref val) if val.len() == 1 => parse_si_f64(&val[0]).chain_err(|| "parsing len")?,
        None => 1.0,
        _ => bail!("len requires exactly one value"),
    };

    ensure!(map.is_empty(), "invalid flags: {:?}", map.keys());

    let sample_rate = parse_si_u64(args.next().ok_or("sample rate argument required")?)
        .chain_err(|| "parsing sample rate")?;

    Ok(Command::Gen {
        sample_rate,
        cos,
        seconds,
    })
}

fn parse_ui<'a, I: Iterator<Item = &'a String>>(
    mut _args: I,
    map: HashMap<String, String>,
) -> Result<Command> {
    ensure!(map.is_empty(), "invalid flags: {:?}", map.keys());

    Ok(Command::Ui)
}

fn guess_sample_rate(filename: &str) -> Option<String> {
    Regex::new(r"\bsr([0-9]+[kMG]?)\b")
        .unwrap()
        .find(filename)
        .map(|s| s.as_str()[2..].to_string())
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

fn parse_si_i64<S: AsRef<str>>(from: S) -> Result<i64> {
    let from = from.as_ref();
    let (val, mul) = find_multiplication_suffix(from);
    let parsed: i64 = val.parse()
        .chain_err(|| format!("parsing signed integer {:?}", from))?;
    Ok(parsed
        .checked_mul(i64::from(mul))
        .ok_or_else(|| format!("unit is out of range: {}", from))?)
}

fn parse_si_u64(from: &str) -> Result<u64> {
    let (val, mul) = find_multiplication_suffix(from);
    let parsed: u64 = val.parse()
        .chain_err(|| format!("parsing unsigned integer {:?}", from))?;
    Ok(parsed
        .checked_mul(u64::from(mul))
        .ok_or_else(|| format!("unit is out of range: {}", from))?)
}

fn parse_si_f64<S: AsRef<str>>(from: S) -> Result<f64> {
    let from = from.as_ref();
    let (val, mul) = find_multiplication_suffix(from);
    let parsed: f64 = val.parse()
        .chain_err(|| format!("parsing float {:?}", from))?;
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

fn guess_from_extension(ext: &str) -> Option<FileFormat> {
    use FileFormat::*;
    Some(match ext {
        "cf32" | "fc32" => ComplexFloat32,
        "cs8" | "sc8" | "c8" => ComplexInt8,
        "cu8" | "su8" => ComplexUint8,
        "cs16" | "sc16" | "c16" => ComplexInt16,

        _ => return None,
    })
}

fn read_just_args<'a, I>(iter: &mut Peekable<I>) -> Result<HashMap<String, Vec<String>>>
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
            Some(arg) if arg.is_empty() => bail!("{} requires a non-empty argument", opt),
            Some(arg) => arg,
            None => bail!("{} requires an argument", opt),
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
