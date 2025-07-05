use std::fs::File;
use std::io::Seek;
use std::io::SeekFrom;

use anyhow::ensure;
use anyhow::Error;
use rustfft::num_complex::Complex;

use crate::usize_from;

pub trait Samples: Sync + Send {
    fn len(&self) -> u64;
    fn sample_rate(&self) -> u64;

    fn read_at(&self, off: u64, buf: &mut [Complex<f32>]) -> usize;

    fn read_exact_at(&self, off: u64, buf: &mut [Complex<f32>]) -> Result<(), Error> {
        let wanted = buf.len();
        let got = self.read_at(off, buf);
        ensure!(
            wanted == got,
            "TODO: read-exact messed up: {} (wanted) != {} (read) at {off}",
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

    fn read_at(&self, off: u64, buf: &mut [Complex<f32>]) -> usize {
        (**self).read_at(off, buf)
    }
}

pub struct SampleFile {
    format: crate::FileFormat,
    file_len: u64,
    inner: File,
    sample_rate: u64,
}

impl SampleFile {
    pub fn new(mut inner: File, format: crate::FileFormat, sample_rate: u64) -> Self {
        let file_len = inner.seek(SeekFrom::End(0)).expect("seeking to end");
        SampleFile {
            inner,
            format,
            file_len,
            sample_rate,
        }
    }
}

impl Samples for SampleFile {
    fn len(&self) -> u64 {
        self.file_len / self.format.pair_bytes()
    }

    fn sample_rate(&self) -> u64 {
        self.sample_rate
    }

    fn read_at(&self, off: u64, into: &mut [Complex<f32>]) -> usize {
        use std::os::unix::fs::FileExt as _;
        assert!(off < self.len());

        let wanted_bytes = (usize_from(self.format.pair_bytes()))
            .checked_mul(into.len())
            .expect("buf too big");
        let mut buf = vec![0u8; wanted_bytes];
        let mut bytes = self
            .inner
            .read_at(&mut buf, off * self.format.pair_bytes())
            .expect("read");
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
