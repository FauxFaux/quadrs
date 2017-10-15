/// Based originally on code from the `synthrs` crate, available under the MIT license.
/// I'm a hustler baby.

use std::f32::consts::PI;

use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;

use usize_from;
use samples::Samples;

pub struct LowPass<S> {
    inner: S,
    filter: Vec<f32>,
    decimate: u64,
    original_sample_rate: u64,
}

impl<S> LowPass<S> {
    pub fn new(
        inner: S,
        frequency: u64,
        decimate: u64,
        original_sample_rate: u64,
        size: usize,
    ) -> Self {
        let cutoff = cutoff_from_frequency(frequency as f64, original_sample_rate);

        let filter = lowpass_filter(cutoff as f32, size);
        LowPass {
            inner,
            filter,
            decimate,
            original_sample_rate,
        }
    }
}

impl<S> Samples for LowPass<S>
where
    S: Samples,
{
    fn len(&self) -> u64 {
        assert!(self.inner.len() >= self.filter.len() as u64);
        1 + (self.inner.len() - self.filter.len() as u64) / self.decimate
    }

    fn sample_rate(&self) -> u64 {
        self.original_sample_rate / self.decimate
    }

    fn read_at(&mut self, off: u64, buf: &mut [Complex<f32>]) -> usize {
        // This will only work well if we feed at least `self.filter.len()` extra
        // samples in before we get to the data we want. We also have to discard samples after the
        // end, and deal with decimation. Right.

        // decimate: 3
        // len: 5
        // input samples: 13
        // underlying: 1bc2ef3hi4kl5
        // output: f(1bc2e), f(2ef3h), f(3hi4k)
        // lost: l, 5
        // 13 - 5 = 8, 8 / 3 = 2
        // 8 - 5 = 3, 3 / 3 = 1

        let underlying_samples = buf.len() * usize_from(self.decimate) + self.filter.len();
        let mut raw_buf = vec![Complex::zero(); underlying_samples];

        let valid = self.inner.read_at(off * self.decimate, &mut raw_buf);
        let convoluted = complex_convolve(&self.filter, &raw_buf[..valid]);

        assert_eq!(self.filter.len() / 2 - 1 + valid, convoluted.len());

        let output_samples = usize_from((valid - self.filter.len()) as u64 / self.decimate);

        for i in 0..output_samples {
            buf[i] = convoluted[self.filter.len() + usize_from(i as u64 * self.decimate)];
        }

        output_samples
    }
}

fn lowpass_filter(cutoff: f32, size: usize) -> Vec<f32> {
    fn sinc(x: f32) -> f32 {
        (x * PI).sin() / (x * PI)
    }

    let blackman_window = (0..size).map(|i| {
        0.42 - 0.5 * (2.0 * PI * i as f32 / (size as f32 - 1.0)).cos() +
            0.08 * (4.0 * PI * i as f32 / (size as f32 - 1.0)).cos()
    });

    let filter: Vec<f32> = (0..size)
        .map(|i| {
            sinc(2.0 * cutoff * (i as f32 - (size as f32 - 1.0) / 2.0))
        })
        .zip(blackman_window)
        .map(|(wave, window)| wave * window)
        .collect();

    // Normalize
    let sum: f32 = filter.iter().sum();
    filter.into_iter().map(|el| el / sum).collect()
}

fn complex_convolve(filter: &[f32], input: &[Complex<f32>]) -> Vec<Complex<f32>> {
    let mut output: Vec<Complex<f32>> = Vec::with_capacity(input.len() + filter.len() / 2);
    let h_len = (filter.len() / 2) as isize;

    for i in -(filter.len() as isize / 2)..(input.len() as isize - 1) {
        output.push(Complex::zero());
        for j in 0isize..filter.len() as isize {
            let input_idx = i + j;
            let output_idx = i + h_len;
            if input_idx < 0 || input_idx >= input.len() as isize {
                continue;
            }
            output[output_idx as usize] += input[input_idx as usize] * filter[j as usize]
        }
    }

    output
}

fn cutoff_from_frequency(frequency: f64, sample_rate: u64) -> f64 {
    frequency / sample_rate as f64
}
