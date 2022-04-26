use core::{str::FromStr, time::Duration};
use s2n_quic::provider::io::testing::rand;

#[derive(Copy, Clone, Debug)]
pub struct CliRange<T> {
    pub start: T,
    pub end: T,
}

impl<T> CliRange<T>
where
    T: Copy + PartialOrd + ::rand::distributions::uniform::SampleUniform,
{
    pub fn gen(&self) -> T {
        if self.start == self.end {
            return self.start;
        }

        rand::gen_range(self.start..self.end)
    }
}

impl CliRange<humantime::Duration> {
    pub fn gen_duration(&self) -> Duration {
        let start = self.start.as_nanos();
        let end = self.end.as_nanos();

        if start == end {
            return Duration::from_nanos(start as _);
        }

        let nanos = rand::gen_range(start..end);
        Duration::from_nanos(nanos as _)
    }
}

impl<T: Copy + FromStr> FromStr for CliRange<T> {
    type Err = T::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((start, end)) = s.split_once("..") {
            let start = start.parse()?;
            let end = end.parse()?;
            Ok(Self { start, end })
        } else {
            let start = s.parse()?;
            let end = start;
            Ok(Self { start, end })
        }
    }
}
