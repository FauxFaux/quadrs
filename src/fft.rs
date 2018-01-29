use std::mem;

use num_complex::Complex;
use num_traits::identities::Zero;
use rustfft::FFT;
use rustfft::algorithm::Radix4;

use errors::*;
use samples::Samples;

pub fn spark_fft(
    samples: &mut Samples,
    fft_width: usize,
    stride: u64,
    min: Option<f32>,
    max: Option<f32>,
) -> Result<()> {
    println!("sparkfft sample_rate={}", samples.sample_rate());

    // TODO: super dumb:
    let min = min.unwrap_or(0.08);
    let max = max.unwrap_or(1.);

    let fft_width = fft_width as usize;

    let fft = Radix4::new(fft_width as usize, false);

    let mut i = 0;
    while i < (samples.len() - fft_width as u64) {
        let mut inp = vec![Complex::zero(); fft_width];
        samples.read_exact_at(i, &mut inp)?;

        let mut out = vec![Complex::zero(); fft_width];

        fft.process(&mut inp, &mut out);
        mem::drop(inp); // inp is now junk

        let top = '█';
        let bot = ' ';
        let graph: Vec<char> = "▁▂▃▄▅▆▇".chars().collect();

        #[cfg(never)]
        let max = out.iter()
            .map(|x| x.norm())
            .max_by(|x, y| x.partial_cmp(y).unwrap())
            .unwrap();

        let distinction = (max - min) / (graph.len() as f32);
        let mut buf = String::with_capacity(fft_width);
        for val in out.iter()
            .skip(fft_width / 2)
            .chain(out.iter().take(fft_width / 2))
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
