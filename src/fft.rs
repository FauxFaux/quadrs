use anyhow::Error;
use num_traits::identities::Zero;
use rustfft::algorithm::Radix4;
use rustfft::num_complex::Complex;
use rustfft::{Fft, FftDirection};

use crate::samples::Samples;

use crate::u64_from;
use crate::usize_from;

pub fn spark_fft(
    samples: &mut dyn Samples,
    fft_width: usize,
    stride: u64,
    min: Option<f32>,
    max: Option<f32>,
) -> Result<(), Error> {
    println!("sparkfft sample_rate={}", samples.sample_rate());

    // TODO: super dumb:
    let min = min.unwrap_or(0.08);
    let max = max.unwrap_or(1.);

    let fft = Radix4::new(fft_width, FftDirection::Forward);

    let mut i = 0;
    while i < (samples.len() - fft_width as u64) {
        let mut inp = vec![Complex::zero(); fft_width];
        samples.read_exact_at(i, &mut inp)?;

        fft.process(&mut inp);

        let top = '█';
        let bot = ' ';
        let graph: Vec<char> = "▁▂▃▄▅▆▇".chars().collect();

        #[cfg(feature = "never")]
        let max = out
            .iter()
            .map(|x| x.norm())
            .max_by(|x, y| x.partial_cmp(y).unwrap())
            .unwrap();

        let distinction = (max - min) / (graph.len() as f32);
        let mut buf = String::with_capacity(fft_width);

        for val in inp
            .iter()
            .skip(fft_width / 2)
            .chain(inp.iter().take(fft_width / 2))
        {
            let norm = val.norm();
            if norm < min {
                buf.push(bot);
            } else if norm >= max {
                buf.push(top);
            } else {
                buf.push(graph[((norm - min) / distinction) as usize]);
            }
        }

        println!("│{}│", buf);

        i += stride;
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct Levels {
    pub vals: Vec<usize>,
}

/// `len/decimate` total to return. Need to read every `decimate`, and for `fft_width`?
pub fn freq_levels(
    samples: &mut dyn Samples,
    fft_width: usize,
    stride: u64,
    levels: usize,
) -> Levels {
    assert_eq!(2, levels, "only supporting two levels for now");

    let fft = Radix4::new(fft_width, FftDirection::Forward);
    let total = (samples.len() - u64_from(fft_width)) / stride;
    let mut vals = Vec::with_capacity(usize_from(total));

    for reading in 0..total {
        let mut inp = vec![Complex::zero(); fft_width];
        samples.read_exact_at(reading * stride, &mut inp).unwrap();

        fft.process(&mut inp);

        let first: f32 = inp.iter().take(fft_width / 2).map(|c| c.norm()).sum();
        let second: f32 = inp.iter().skip(fft_width / 2).map(|c| c.norm()).sum();
        vals.push(if first < second { 0 } else { 1 });
    }

    Levels { vals }
}
