use crate::usize_from;

fn decode(data: &[bool]) -> String {
    let mut data = data.into_iter().cloned();
    let mut s = String::with_capacity(data.len() / 8);
    'a: loop {
        let mut val = 0u8;
        for bit in (0..8).rev() {
            match data.next() {
                Some(true) => val |= 1 << bit,
                Some(false) => (),
                None => break 'a,
            }
        }
        s.push(val as char);
    }

    s
}

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

    #[test]
    fn de_run() {
        use super::decode;
        use super::scan;

        // 1100000110010100110000011111110100101100011101010001000001000001
        let inp = parse(
            r#"11
                 010000000111110000011111000001111100001111100000111110000011111000001111100001
                 111100000111110000011111000001111100000111100000111110000011111000001111100000
                 111110000111110000011111000001111100000111110000111110000011111000001111100000
                 111110000111110000011111000001111100000111110000011110000011111000001111100000
                 11111000001111111110000000000000000000000001111111111000000000111110000011111
                 000000000011111111110000000000000000000000001111111111111111111111111111111111
                 000001111100000000001111100001111111111000000000000000111111111111111000011111
                 000001111100000000000000011111000000000000000000000000111110000000000000000000
                 00000111111110101"#,
        );

        let (_err, val) = scan(&inp, 4.8);
        println!("{}", super::fmt(&val));
        for off in 0..8 {
            println!("{:?}", decode(&val[off..]));
        }

        let inv: Vec<bool> = val.iter().map(|&x| !x).collect();
        for off in 0..8 {
            println!("{:?}", decode(&inv[off..]));
        }
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

fn fmt(bits: &[bool]) -> String {
    bits.iter().map(|&x| if x { 'X' } else { '.' }).collect()
}
