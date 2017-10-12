use std::f64::consts::PI;

use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;

use usize_from;
use samples::Samples;

const TAU: f64 = PI * 2.;

pub struct Shift<S> {
    inner: S,
    ratio: f64,
}

impl<S> Shift<S> {
    /// frequency: complete waves per second
    /// sample rate: samples per second

    pub fn new(inner: S, frequency: i64, sample_rate: u64) -> Self {
        assert!(
            frequency < (sample_rate / 2) as i64,
            "frequency must be under half the sample rate"
        );
        assert!(sample_rate > 0);

        Shift {
            inner,
            ratio: TAU * (frequency as f64) / (sample_rate as f64),
        }
    }
}

impl<S> Samples for Shift<S>
where
    S: Samples,
{
    fn len(&self) -> u64 {
        self.inner.len()
    }

    fn read_at(&mut self, off: u64, buf: &mut [Complex<f32>]) -> usize {
        let valid = self.inner.read_at(off, buf);
        for i in 0..valid {
            let place = (off + (i as u64)) as f64 * self.ratio;
            let mul = Complex::new(place.cos() as f32, place.sin() as f32);
            buf[i] *= mul;
        }
        valid
    }
}
