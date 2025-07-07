use std::{fs, io};
use anyhow::{bail, Result};

fn main() -> Result<()> {
    for (li, line) in io::stdin().lines().enumerate() {
        let line = line?;
        let mut bits = Vec::<u8>::new();
        for c in line.chars() {
            match c {
                '0' => bits.extend_from_slice([0, 0, 0, 0].as_ref()),
                '1' => bits.extend_from_slice([0, 0, 0, 1].as_ref()),
                '2' => bits.extend_from_slice([0, 0, 1, 0].as_ref()),
                '3' => bits.extend_from_slice([0, 0, 1, 1].as_ref()),
                '4' => bits.extend_from_slice([0, 1, 0, 0].as_ref()),
                '5' => bits.extend_from_slice([0, 1, 0, 1].as_ref()),
                '6' => bits.extend_from_slice([0, 1, 1, 0].as_ref()),
                '7' => bits.extend_from_slice([0, 1, 1, 1].as_ref()),
                '8' => bits.extend_from_slice([1, 0, 0, 0].as_ref()),
                '9' => bits.extend_from_slice([1, 0, 0, 1].as_ref()),
                'A' | 'a' => bits.extend_from_slice([1, 0, 1, 0].as_ref()),
                'B' | 'b' => bits.extend_from_slice([1, 0, 1, 1].as_ref()),
                'C' | 'c' => bits.extend_from_slice([1, 1, 0, 0].as_ref()),
                'D' | 'd' => bits.extend_from_slice([1, 1, 0, 1].as_ref()),
                'E' | 'e' => bits.extend_from_slice([1, 1, 1, 0].as_ref()),
                'F' | 'f' => bits.extend_from_slice([1, 1, 1, 1].as_ref()),
                other => bail!("Invalid character in input: {other:?}"),
            }
        }

        println!("line: {line}");

        let key = b"GROWATTRF.";

        for i_skip in 0..8 {
            let chars = bits[i_skip..].chunks_exact(8).map(to_byte).map(char::from).collect::<String>();
            println!("decode {i_skip}: {chars:?}");
            for i_key in 0..key.len() {
                let decoded = chars.bytes().zip(key.iter().cycle().skip(i_key)).map(|(a, b)| a ^ *b).collect::<Vec<u8>>();
                let display = decoded.iter()
                    .map(|&b| if b.is_ascii_graphic() { b } else if b == 0 { b' ' } else { b'.' })
                    .map(char::from).collect::<String>();
                println!(" - {i_key}: {display:?}");

                if decoded.chunks_exact(10).any(|chunk| chunk.iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())) {
                    println!("  - found key at {i_key}");
                    println!("  - decoded: {decoded:?}");
                    fs::write(format!("decoded_{li}_{i_skip}_{i_key}.bin"), decoded)?;
                }
            }
        }
        println!();
        println!();
        println!();
    }

    Ok(())
}

fn to_byte(bits: &[u8]) -> u8 {
    bits.iter().fold(0, |acc, &bit| (acc << 1) | bit)
}
