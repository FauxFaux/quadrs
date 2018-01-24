use rustfft::num_complex::Complex;
use rustfft::num_traits::identities::Zero;

use errors::*;
use samples::Samples;

use TAU;

pub struct Gen {
    sample_rate: u64,
    seconds: f64,
    cos: Vec<i64>,
}

impl Gen {
    pub fn new(cos: Vec<i64>, sample_rate: u64, seconds: f64) -> Result<Self> {
        ensure!(!cos.is_empty(), "cos cannot be empty");
        ensure!(0 != sample_rate, "sample rate may not be zero");
        ensure!(seconds > 0.0, "seconds may not be <= 0");

        Ok(Gen {
            cos,
            sample_rate,
            seconds,
        })
    }
}

impl Samples for Gen {
    fn len(&self) -> u64 {
        (self.seconds * (self.sample_rate as f64)) as u64
    }

    fn read_at(&mut self, off: u64, buf: &mut [Complex<f32>]) -> usize {
        for i in 0..buf.len() {
            let base = (off + (i as u64)) as f64 * TAU / self.sample_rate as f64;
            let mut val = Complex::zero();
            for freq in &self.cos {
                let f = (*freq as f64) * base;
                val += Complex::new(f.cos() as f32, f.sin() as f32);
            }
            buf[i] = val;
        }

        buf.len()
    }

    fn sample_rate(&self) -> u64 {
        self.sample_rate
    }
}
