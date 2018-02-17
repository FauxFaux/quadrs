use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;

use rustfft::num_complex::Complex;

use errors::*;
use usize_from;

pub trait Samples {
    fn len(&self) -> u64;
    fn sample_rate(&self) -> u64;

    fn read_at(&mut self, off: u64, buf: &mut [Complex<f32>]) -> usize;

    fn read_exact_at(&mut self, off: u64, buf: &mut [Complex<f32>]) -> Result<()> {
        let wanted = buf.len();
        let got = self.read_at(off, buf);
        ensure!(
            wanted == got,
            "TODO: read-exact messed up: {} (wanted) != {} (read)",
            wanted,
            got
        );
        Ok(())
    }
}

impl<T: Samples + ?Sized> Samples for Box<T> {
    fn len(&self) -> u64 {
        (**self).len()
    }

    fn sample_rate(&self) -> u64 {
        (**self).sample_rate()
    }

    fn read_at(&mut self, off: u64, buf: &mut [Complex<f32>]) -> usize {
        (**self).read_at(off, buf)
    }
}

pub struct SampleFile<R> {
    format: ::FileFormat,
    file_len: u64,
    inner: R,
    sample_rate: u64,
}

impl<R> SampleFile<R>
where
    R: Read + Seek,
{
    pub fn new(mut inner: R, format: ::FileFormat, sample_rate: u64) -> Self {
        let file_len = inner.seek(SeekFrom::End(0)).expect("seeking to end");
        SampleFile {
            inner,
            format,
            file_len,
            sample_rate,
        }
    }
}

impl<R> Samples for SampleFile<R>
where
    R: Read + Seek,
{
    fn len(&self) -> u64 {
        self.file_len / self.format.pair_bytes()
    }

    fn sample_rate(&self) -> u64 {
        self.sample_rate
    }

    fn read_at(&mut self, off: u64, into: &mut [Complex<f32>]) -> usize {
        assert!(off < self.len());
        self.inner
            .seek(SeekFrom::Start(off * self.format.pair_bytes()))
            .expect("seek");

        let wanted_bytes = (usize_from(self.format.pair_bytes()))
            .checked_mul(into.len())
            .expect("buf too big");
        let mut buf = vec![0u8; wanted_bytes];
        let mut bytes = self.inner.read(&mut buf).expect("read");
        bytes -= bytes % usize_from(self.format.pair_bytes());
        for (i, sample) in buf[0..bytes]
            .chunks(usize_from(self.format.pair_bytes()))
            .enumerate()
        {
            into[i] = self.format.to_cf32(sample);
        }

        (bytes) / usize_from(self.format.pair_bytes())
    }
}
