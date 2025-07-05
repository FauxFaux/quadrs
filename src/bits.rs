use crate::usize_from;

pub fn scan(data: &[bool], scale: f64) -> (f64, Vec<bool>) {
    let mut i = 0;
    let half = usize_from((scale / 2.).round() as u64);
    let mut bit = false;
    let mut error = 0.;
    let mut ret = Vec::with_capacity(data.len() / usize_from((scale + 2.) as u64));
    while i != data.len() {
        let found = run_of(&data[i..], half, bit);
        i += found;

        if found <= half {
            continue;
        }

        let bits = (found as f64) / scale;

        #[cfg(feature = "never")]
        println!(
            "{:0.2} {} {}",
            bits,
            if bit { "X" } else { "." },
            fmt(&data[(i - found)..])
        );

        let rounded = bits.round();
        error += (bits - rounded).abs();

        for _ in 0..(rounded as u64) {
            ret.push(bit);
        }

        bit = !bit;
    }

    (error, ret)
}

fn run_of(data: &[bool], scale: usize, val: bool) -> usize {
    let mut bad = 0;
    for (i, bit) in data.iter().cloned().enumerate() {
        if bit != val {
            bad += 1;
        } else {
            bad = 0;
        }

        if bad > scale {
            return i + 1 - bad;
        }
    }

    data.len()
}

#[cfg(test)]
mod tests {

    #[test]
    fn run() {
        use super::run_of;
        assert_eq!(4, run_of(&parse("0000"), 2, false), "runs a whole buffer");
        assert_eq!(
            8,
            run_of(&parse("00001000111"), 2, false),
            "doesn't trip over a single bit flip at 2"
        );
    }

    fn parse(s: &str) -> Vec<bool> {
        s.chars()
            .flat_map(|x| match x {
                '0' => Some(false),
                '1' => Some(true),
                x if x.is_whitespace() => None,
                _ => panic!("invalid"),
            })
            .collect()
    }
}
