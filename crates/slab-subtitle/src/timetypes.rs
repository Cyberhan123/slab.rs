// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

/// Represents a timepoint (e.g. start timepoint of a subtitle line).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Timing(i64 /* number of milliseconds */);

/// The internal timing in `TimePoint` and `TimeDelta` (with all necessary functions and nice Debug information, etc.).
impl Timing {
    fn from_components(hours: i64, mins: i64, secs: i64, ms: i64) -> Timing {
        Timing(ms + 1000 * (secs + 60 * (mins + 60 * hours)))
    }

    fn from_msecs(ms: i64) -> Timing {
        Timing(ms)
    }

    fn from_csecs(cs: i64) -> Timing {
        Timing(cs * 10)
    }

    fn from_secs(s: i64) -> Timing {
        Timing(s * 1000)
    }

    fn from_mins(mins: i64) -> Timing {
        Timing(mins * 1000 * 60)
    }

    fn from_hours(h: i64) -> Timing {
        Timing(h * 1000 * 60 * 60)
    }

    fn msecs(&self) -> i64 {
        self.0
    }

    fn csecs(&self) -> i64 {
        self.0 / 10
    }

    fn secs(&self) -> i64 {
        self.0 / 1000
    }

    fn secs_f64(&self) -> f64 {
        self.0 as f64 / 1000.0
    }

    fn mins(&self) -> i64 {
        self.0 / (60 * 1000)
    }

    fn hours(&self) -> i64 {
        self.0 / (60 * 60 * 1000)
    }

    fn mins_comp(&self) -> i64 {
        self.mins() % 60
    }

    fn secs_comp(&self) -> i64 {
        self.secs() % 60
    }

    fn csecs_comp(&self) -> i64 {
        self.csecs() % 100
    }

    fn msecs_comp(&self) -> i64 {
        self.msecs() % 1000
    }

    fn is_negative(&self) -> bool {
        self.0 < 0
    }
}

impl Debug for Timing {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "Timing({})", self)
    }
}

impl Display for Timing {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        let t = if self.0 < 0 { -*self } else { *self };
        write!(
            f,
            "{}{}:{:02}:{:02}.{:03}",
            if self.0 < 0 { "-" } else { "" },
            t.hours(),
            t.mins_comp(),
            t.secs_comp(),
            t.msecs_comp()
        )
    }
}

impl Add for Timing {
    type Output = Timing;
    fn add(self, rhs: Timing) -> Timing {
        Timing(self.0 + rhs.0)
    }
}

impl Sub for Timing {
    type Output = Timing;
    fn sub(self, rhs: Timing) -> Timing {
        Timing(self.0 - rhs.0)
    }
}

impl AddAssign for Timing {
    fn add_assign(&mut self, r: Timing) {
        self.0 += r.0;
    }
}

impl SubAssign for Timing {
    fn sub_assign(&mut self, r: Timing) {
        self.0 -= r.0;
    }
}

impl Neg for Timing {
    type Output = Timing;
    fn neg(self) -> Timing {
        Timing(-self.0)
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
/// Represents a time point like the start time of a subtitle entry.
pub struct TimePoint {
    /// The internal timing (with all necessary functions and nice Debug information, etc.).
    intern: Timing,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
/// Represents a duration between two `TimePoints`.
pub struct TimeDelta {
    /// The internal timing (with all necessary functions and nice Debug information, etc.).
    intern: Timing,
}

macro_rules! create_time_type {
    ($i:ident) => {
        impl $i {
            fn new(t: Timing) -> $i {
                $i { intern: t }
            }

            /// Create this time type from all time components.
            ///
            /// The components can be negative and/or exceed the its natural limits without error.
            /// For example `from_components(0, 0, 3, -2000)` is the same as `from_components(0, 0, 1, 0)`.
            pub fn from_components(hours: i64, mins: i64, secs: i64, ms: i64) -> $i {
                Self::new(Timing::from_components(hours, mins, secs, ms))
            }

            /// Create the time type from a given number of milliseconds.
            pub fn from_msecs(ms: i64) -> $i {
                Self::new(Timing::from_msecs(ms))
            }

            /// Create the time type from a given number of hundreth seconds (10 milliseconds).
            pub fn from_csecs(ms: i64) -> $i {
                Self::new(Timing::from_csecs(ms))
            }

            /// Create the time type with a given number of seconds.
            pub fn from_secs(ms: i64) -> $i {
                Self::new(Timing::from_secs(ms))
            }

            /// Create the time type with a given number of minutes.
            pub fn from_mins(mins: i64) -> $i {
                Self::new(Timing::from_mins(mins))
            }

            /// Create the time type with a given number of hours.
            pub fn from_hours(mins: i64) -> $i {
                Self::new(Timing::from_hours(mins))
            }

            /// Get the total number of milliseconds.
            pub fn msecs(&self) -> i64 {
                self.intern.msecs()
            }

            /// Get the total number of hundreth seconds.
            pub fn csecs(&self) -> i64 {
                self.intern.csecs()
            }

            /// Get the total number of seconds.
            pub fn secs(&self) -> i64 {
                self.intern.secs()
            }

            /// Get the total number of seconds.
            pub fn secs_f64(&self) -> f64 {
                self.intern.secs_f64()
            }

            /// Get the total number of seconds.
            pub fn mins(&self) -> i64 {
                self.intern.mins()
            }

            /// Get the total number of hours.
            pub fn hours(&self) -> i64 {
                self.intern.hours()
            }

            /// Get the milliseconds component in a range of [0, 999].
            pub fn msecs_comp(&self) -> i64 {
                self.intern.msecs_comp()
            }

            /// Get the hundreths seconds component in a range of [0, 99].
            pub fn csecs_comp(&self) -> i64 {
                self.intern.csecs_comp()
            }

            /// Get the seconds component in a range of [0, 59].
            pub fn secs_comp(&self) -> i64 {
                self.intern.secs_comp()
            }

            /// Get the minute component in a range of [0, 59].
            pub fn mins_comp(&self) -> i64 {
                self.intern.mins_comp()
            }

            /// Return `true` if the represented time is negative.
            pub fn is_negative(&self) -> bool {
                self.intern.is_negative()
            }

            /// Return the absolute value of the current time.
            pub fn abs(&self) -> $i {
                if self.is_negative() { -*self } else { *self }
            }
        }

        impl Neg for $i {
            type Output = $i;
            fn neg(self) -> $i {
                $i::new(-self.intern)
            }
        }

        impl Display for $i {
            fn fmt(&self, f: &mut Formatter) -> FmtResult {
                write!(f, "{}", self.intern)
            }
        }
    };
}

create_time_type! {TimePoint}
create_time_type! {TimeDelta}

macro_rules! impl_add {
    ($a:ty, $b:ty, $output:ident) => {
        impl Add<$b> for $a {
            type Output = $output;
            fn add(self, rhs: $b) -> $output {
                $output::new(self.intern + rhs.intern)
            }
        }
    };
}

macro_rules! impl_sub {
    ($a:ty, $b:ty, $output:ident) => {
        impl Sub<$b> for $a {
            type Output = $output;
            fn sub(self, rhs: $b) -> $output {
                $output::new(self.intern - rhs.intern)
            }
        }
    };
}

macro_rules! impl_add_assign {
    ($a:ty, $b:ty) => {
        impl AddAssign<$b> for $a {
            fn add_assign(&mut self, r: $b) {
                self.intern += r.intern;
            }
        }
    };
}

macro_rules! impl_sub_assign {
    ($a:ty, $b:ty) => {
        impl SubAssign<$b> for $a {
            fn sub_assign(&mut self, r: $b) {
                self.intern -= r.intern;
            }
        }
    };
}

impl_add!(TimeDelta, TimeDelta, TimeDelta);
impl_add!(TimePoint, TimeDelta, TimePoint);
impl_add!(TimeDelta, TimePoint, TimePoint);

impl_sub!(TimeDelta, TimeDelta, TimeDelta);
impl_sub!(TimePoint, TimePoint, TimeDelta);
impl_sub!(TimePoint, TimeDelta, TimePoint);
impl_sub!(TimeDelta, TimePoint, TimePoint);

impl_add_assign!(TimeDelta, TimeDelta);
impl_add_assign!(TimePoint, TimeDelta);

impl_sub_assign!(TimeDelta, TimeDelta);
impl_sub_assign!(TimePoint, TimeDelta);

/// A time span (e.g. time in which a subtitle is shown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeSpan {
    /// Start of the time span.
    pub start: TimePoint,

    /// End of the time span.
    pub end: TimePoint,
}

impl TimeSpan {
    /// Constructor of `TimeSpan`s.
    pub fn new(start: TimePoint, end: TimePoint) -> TimeSpan {
        TimeSpan { start, end }
    }

    /// Get the length of the `TimeSpan` (can be negative).
    pub fn len(&self) -> TimeDelta {
        self.end - self.start
    }
}

impl Add<TimeDelta> for TimeSpan {
    type Output = TimeSpan;
    fn add(self, rhs: TimeDelta) -> TimeSpan {
        TimeSpan::new(self.start + rhs, self.end + rhs)
    }
}

impl Sub<TimeDelta> for TimeSpan {
    type Output = TimeSpan;
    fn sub(self, rhs: TimeDelta) -> TimeSpan {
        TimeSpan::new(self.start - rhs, self.end - rhs)
    }
}

impl AddAssign<TimeDelta> for TimeSpan {
    fn add_assign(&mut self, r: TimeDelta) {
        self.start += r;
        self.end += r;
    }
}

impl SubAssign<TimeDelta> for TimeSpan {
    fn sub_assign(&mut self, r: TimeDelta) {
        self.start -= r;
        self.end -= r;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timing_display() {
        let t = -super::Timing::from_components(12, 59, 29, 450);
        assert_eq!(t.to_string(), "-12:59:29.450".to_string());

        let t = super::Timing::from_msecs(0);
        assert_eq!(t.to_string(), "0:00:00.000".to_string());
    }

    #[test]
    fn test_timepoint_from_components_normalizes() {
        // Test that components are normalized (60 seconds = 1 minute, etc.)
        let tp = TimePoint::from_components(0, 0, 0, 3661000); // 61 minutes + 1 second in ms
        assert_eq!(tp.mins(), 61);
        assert_eq!(tp.secs_comp(), 1);
    }

    #[test]
    fn test_timepoint_negative() {
        let tp = TimePoint::from_msecs(-1000);
        assert!(tp.is_negative());
        assert_eq!(tp.abs().msecs(), 1000);
    }

    #[test]
    fn test_timepoint_addition() {
        let tp = TimePoint::from_secs(60);
        let td = TimeDelta::from_secs(30);
        let result = tp + td;
        assert_eq!(result.secs(), 90);
    }

    #[test]
    fn test_timepoint_subtraction() {
        let tp1 = TimePoint::from_secs(60);
        let tp2 = TimePoint::from_secs(30);
        let result = tp1 - tp2; // This returns TimeDelta
        assert_eq!(result.secs(), 30);
    }

    #[test]
    fn test_timepoint_add_time_delta() {
        let tp = TimePoint::from_secs(60);
        let td = TimeDelta::from_secs(30);
        let result = tp + td;
        assert_eq!(result.secs(), 90);
    }

    #[test]
    fn test_timedelta_from_components() {
        let td = TimeDelta::from_components(1, 30, 45, 500);
        assert_eq!(td.secs(), 5445); // 1*3600 + 30*60 + 45
        assert_eq!(td.msecs_comp(), 500);
    }

    #[test]
    fn test_timedelta_negative() {
        let td = TimeDelta::from_secs(-60);
        assert!(td.is_negative());
        assert_eq!(td.abs().secs(), 60);
    }

    #[test]
    fn test_timespan_length() {
        let start = TimePoint::from_secs(10);
        let end = TimePoint::from_secs(20);
        let span = TimeSpan::new(start, end);
        assert_eq!(span.len().secs(), 10);
    }

    #[test]
    fn test_timespan_negative_length() {
        let start = TimePoint::from_secs(20);
        let end = TimePoint::from_secs(10);
        let span = TimeSpan::new(start, end);
        assert_eq!(span.len().secs(), -10);
    }

    #[test]
    fn test_timespan_add_timedelta() {
        let start = TimePoint::from_secs(10);
        let end = TimePoint::from_secs(20);
        let span = TimeSpan::new(start, end);
        let offset = TimeDelta::from_secs(5);

        let result = span + offset;
        assert_eq!(result.start.secs(), 15);
        assert_eq!(result.end.secs(), 25);
    }

    #[test]
    fn test_timespan_subtract_timedelta() {
        let start = TimePoint::from_secs(10);
        let end = TimePoint::from_secs(20);
        let span = TimeSpan::new(start, end);
        let offset = TimeDelta::from_secs(5);

        let result = span - offset;
        assert_eq!(result.start.secs(), 5);
        assert_eq!(result.end.secs(), 15);
    }

    #[test]
    fn test_timepoint_comparison() {
        let tp1 = TimePoint::from_secs(10);
        let tp2 = TimePoint::from_secs(20);

        assert!(tp1 < tp2);
        assert!(tp2 > tp1);
        assert_eq!(tp1, tp1);
    }

    #[test]
    fn test_timepoint_equality() {
        let tp1 = TimePoint::from_msecs(1000);
        let tp2 = TimePoint::from_secs(1);
        assert_eq!(tp1, tp2);
    }

    #[test]
    fn test_timepoint_max_values() {
        // Test very large timestamps (near boundaries)
        let tp = TimePoint::from_hours(99);
        assert_eq!(tp.hours(), 99);
        assert_eq!(tp.mins_comp(), 0);
    }

    #[test]
    fn test_timedelta_add_assign() {
        let mut td = TimeDelta::from_secs(60);
        td += TimeDelta::from_secs(30);
        assert_eq!(td.secs(), 90);
    }

    #[test]
    fn test_timedelta_sub_assign() {
        let mut td = TimeDelta::from_secs(90);
        td -= TimeDelta::from_secs(30);
        assert_eq!(td.secs(), 60);
    }

    #[test]
    fn test_timespan_add_assign_timedelta() {
        let mut span = TimeSpan::new(TimePoint::from_secs(10), TimePoint::from_secs(20));
        span += TimeDelta::from_secs(5);
        assert_eq!(span.start.secs(), 15);
        assert_eq!(span.end.secs(), 25);
    }

    #[test]
    fn test_timespan_sub_assign_timedelta() {
        let mut span = TimeSpan::new(TimePoint::from_secs(10), TimePoint::from_secs(20));
        span -= TimeDelta::from_secs(5);
        assert_eq!(span.start.secs(), 5);
        assert_eq!(span.end.secs(), 15);
    }
}
