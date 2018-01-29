use std::mem;

use num_complex::Complex;
use num_traits::identities::Zero;
use rustfft::FFT;
use rustfft::algorithm::Radix4;

use errors::*;
use samples::Bits;
use samples::Samples;

use usize_from;

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

pub struct FreqSlicer<S: Samples> {
    inner: S,
    fft_width: usize,
    decimate: u64,
}

impl<S: Samples> Bits for FreqSlicer<S> {
    fn len(&self) -> u64 {
        self.inner.len() / self.decimate
    }

    /// `len/decimate` total to return. Need to read every `decimate`, and for fft_width?
    fn read_at(&mut self, off: u64, buf: &mut [bool]) -> usize {
        let fft = Radix4::new(self.fft_width, false);
        let start = self.decimate * off;
        let len = buf.len();
        let mut buf = vec![Complex::zero(); len * usize_from(self.decimate) + self.fft_width];
        let valid = self.inner.read_at(start, &mut buf);
        let buf = &buf[..valid];
        for i in 0..len.min(buf.len() / usize_from(self.decimate)) {
            let mut out = vec![Complex::zero(); self.fft_width];
            // crap, the fft corrupts our buffer, so we can't re-use overlapping reads anyway
            let copy = &buf[i * usize_from(self.decimate)..];
            let copy: &[Complex<f32>] = &copy[..self.fft_width];
            let mut copy = copy.to_vec();
            fft.process(&mut copy, &mut out);
            let first: f32 = out.iter().take(self.fft_width / 2).map(|c| c.norm()).sum();
            let second: f32 = out.iter().skip(self.fft_width / 2).map(|c| c.norm()).sum();
            println!("{} {}", first, second);
        }

        0
    }

    fn sample_rate(&self) -> u64 {
        self.inner.sample_rate() / self.decimate
    }
}
