//! The module contains [`DisplayableRemainingTime`] type and its methods.

use std::fmt::Display;

use thiserror::Error;
use time::ext::NumericalDuration as _;

#[derive(Debug, Clone, Copy)]
pub struct DisplayableRemainingTime {
    pub mins: usize,
    pub secs: usize,
}

impl Display for DisplayableRemainingTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.mins, self.secs)
    }
}

#[derive(Debug, Error)]
#[error("given duration {0} is negative")]
pub struct TryFromDurationError(prost_types::Duration);

impl TryFrom<prost_types::Duration> for DisplayableRemainingTime {
    type Error = TryFromDurationError;

    fn try_from(value: prost_types::Duration) -> Result<Self, Self::Error> {
        let dur = time::SignedDuration::new(value.seconds, value.nanos);
        if dur.is_negative() {
            return Err(TryFromDurationError(value));
        }
        let mins = dur.whole_minutes();
        let submin_dur = dur - mins.minutes();
        let secs = submin_dur.whole_seconds();
        Ok(Self {
            // dur is positive as we checked above
            mins: mins.try_into().unwrap(),
            secs: secs.try_into().unwrap(),
        })
    }
}

impl DisplayableRemainingTime {
    pub const ZERO: Self = DisplayableRemainingTime { mins: 0, secs: 0 };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_from_durarion_calculates_correctly() {
        let mins = 2usize;
        let secs = 20usize;
        let dur = prost_types::Duration {
            seconds: (60 * mins + secs).try_into().unwrap(),
            nanos: 0,
        };
        let rem = DisplayableRemainingTime::try_from(dur).unwrap();
        assert_eq!(rem.mins, mins);
        assert_eq!(rem.secs, secs);
    }

    #[test]
    fn try_from_duration_returns_err_when_dur_is_negative() {
        let dur = prost_types::Duration {
            seconds: -1,
            nanos: -500,
        };
        DisplayableRemainingTime::try_from(dur).expect_err("should return error");
    }

    #[test]
    fn remaining_time_is_displayed_with_zero_filling() {
        let rem = DisplayableRemainingTime { mins: 1, secs: 9 };
        let s = format!("{}", rem);
        assert_eq!(&s, "01:09");
    }
}
