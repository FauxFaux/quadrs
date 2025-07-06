use crate::{u64_from, Samples};
use anyhow::{ensure, Result};
use num_traits::Zero;
use rustfft::num_complex::Complex;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct FftConfig {
    pub width: usize,
    pub windowing: Windowing,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Windowing {
    Rectangular,
    BlackmanHarris,
}

pub fn take_fft(
    samples: &dyn Samples,
    slice: Option<(u64, u64)>,
    config: &FftConfig,
    output_len: usize,
) -> Result<FftResult> {
    let fft_width = config.width;
    let fft = rustfft::FftPlanner::<f32>::new().plan_fft_forward(fft_width);

    let (start_sample, end_sample) = match slice {
        Some((start, end)) => (start, end),
        None => (0, samples.len() - u64_from(fft_width)),
    };

    assert!(
        end_sample > start_sample,
        "Invalid slice: end ({end_sample}) must be greater than start ({start_sample})"
    );
    assert!(
        end_sample < samples.len(),
        "Slice end ({end_sample}) exceeds sample length ({})",
        samples.len()
    );

    let mut buf: Vec<f32> = Vec::with_capacity(output_len * fft_width);

    let visible_samples = end_sample - start_sample;
    ensure!(
        visible_samples > u64_from(output_len),
        "Visible samples ({visible_samples}) must be greater than output length ({output_len})"
    );

    let step = visible_samples as f64 / output_len as f64;
    let mut complex_buf = vec![Complex::zero(); fft_width];
    let mut scratch = vec![Complex::zero(); fft.get_inplace_scratch_len()];

    let window = match config.windowing {
        Windowing::BlackmanHarris => Some(generate_blackman_harris_window(fft_width)),
        Windowing::Rectangular => None,
    };

    for i in 0..output_len {
        let sample_index = start_sample + (step * i as f64).round() as u64;

        samples.read_exact_at(sample_index, &mut complex_buf)?;

        if let Some(ref w) = window {
            for (sample, &w_val) in complex_buf.iter_mut().zip(w.iter()) {
                *sample *= w_val;
            }
        }

        fft.process_with_scratch(&mut complex_buf, &mut scratch);

        for val in complex_buf
            .iter()
            .skip(fft_width / 2)
            .chain(complex_buf.iter().take(fft_width / 2))
        {
            buf.push(val.norm());
        }
    }

    Ok(FftResult {
        inner: buf.into_boxed_slice(),
        fft_width,
    })
}
pub struct FftResult {
    inner: Box<[f32]>,
    fft_width: usize,
}

impl FftResult {
    pub fn get(&self, index: usize) -> &[f32] {
        assert!(index < self.output_len(), "index out of bounds: {}", index);
        &self.inner[index * self.fft_width..(index + 1) * self.fft_width]
    }

    pub fn output_len(&self) -> usize {
        self.inner.len() / self.fft_width
    }

    pub fn max(&self) -> f32 {
        self.inner.iter().cloned().fold(0.0, f32::max)
    }

    pub fn min(&self) -> f32 {
        self.inner.iter().cloned().fold(f32::INFINITY, f32::min)
    }
}

fn generate_blackman_harris_window(n: usize) -> Vec<f32> {
    let mut window = Vec::with_capacity(n);
    for i in 0..n {
        let x = std::f32::consts::TAU * i as f32 / (n - 1) as f32;
        let value =
            0.35875 - 0.48829 * x.cos() + 0.14128 * (2.0 * x).cos() - 0.01168 * (3.0 * x).cos();
        window.push(value);
    }
    window
}
