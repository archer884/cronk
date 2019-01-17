use chrono::{Date, DateTime, Datelike, Local, LocalResult, TimeZone, Timelike};
use std::cmp;
use std::ops;

trait Ticker: Sized + ops::Add + ops::AddAssign + cmp::PartialOrd {
    fn increment(&self) -> Self;
}

impl Ticker for u8 {
    fn increment(&self) -> u8 {
        self + 1
    }
}

impl Ticker for i32 {
    fn increment(&self) -> i32 {
        self + 1
    }
}

pub enum Field<T> {
    Single(T),
    Multiple(Vec<T>),
    Range(T, T),
}

impl<T: Copy> Field<T> {
    fn seed(&self) -> T {
        match *self {
            Field::Single(seed) => seed,
            Field::Multiple(ref set) => *set.first().unwrap(),
            Field::Range(seed, _) => seed,
        }
    }

    fn into_increment(self) -> Increment<T> {
        match self {
            Field::Single(x) => Increment::Single(x),
            Field::Multiple(set) => Increment::Multiple(SetTicker::new(set)),
            Field::Range(min, max) => Increment::Range(RangeTicker::new(min, max)),
        }
    }
}

pub struct Weekday {
    pub field: Field<u8>,
    pub nth: Option<Nth>,
}

pub struct Nth(u8);

impl Weekday {
    fn is_valid<Z: TimeZone>(&self, date: &Date<Z>) -> bool {
        let day_of_week = date.weekday().num_days_from_sunday() as u8;
        let is_correct_day = match self.field {
            Field::Single(x) => x == day_of_week,
            Field::Multiple(ref set) => set.contains(&day_of_week),
            Field::Range(min, max) => min <= day_of_week && max >= day_of_week,
        };

        match self.nth {
            None => is_correct_day,
            Some(Nth(n)) => match date.day() % 7 {
                0 => is_correct_day && n == (date.day() as u8 / 7),
                _ => is_correct_day && n == (date.day() as u8 / 7 + 1),
            }
        }
    }
}

pub struct Expression {
    pub minute: Option<Field<u8>>,
    pub hour: Option<Field<u8>>,
    pub dom: Option<Field<u8>>,
    pub month: Option<Field<u8>>,
    pub dow: Option<Weekday>,
}

impl Expression {
    pub fn into_schedule(self) -> Schedule {
        let local = Local::now();

        let minute = self
            .minute
            .as_ref()
            .map(|x| x.seed())
            .unwrap_or_else(|| local.minute() as u8);

        let hour = self
            .hour
            .as_ref()
            .map(|x| x.seed())
            .unwrap_or_else(|| local.hour() as u8);

        let day = self
            .dom
            .as_ref()
            .map(|x| x.seed())
            .unwrap_or_else(|| local.day() as u8);

        let month = self
            .month
            .as_ref()
            .map(|x| x.seed())
            .unwrap_or_else(|| local.month() as u8);

        let current = CandidateDateTime {
            minute,
            hour,
            day,
            month,
            year: local.year(),
        };

        let increment_minute = self.minute.map(Field::into_increment).unwrap_or_else(|| {
            Increment::Range(RangeTicker::with_current(0, 59, local.minute() as u8))
        });

        let increment_hour = self.hour.map(Field::into_increment).unwrap_or_else(|| {
            Increment::Range(RangeTicker::with_current(0, 23, local.hour() as u8))
        });

        let increment_dom = self.dom.map(Field::into_increment).unwrap_or_else(|| {
            Increment::Range(RangeTicker::with_current(1, 31, local.day() as u8))
        });

        let increment_month = self.month.map(Field::into_increment).unwrap_or_else(|| {
            Increment::Range(RangeTicker::with_current(1, 12, local.month() as u8))
        });

        Schedule {
            current,
            increment_minute,
            increment_hour,
            increment_dom,
            increment_month,
            dow: self.dow,
        }
    }
}

pub struct Schedule {
    current: CandidateDateTime,
    increment_minute: Increment<u8>,
    increment_hour: Increment<u8>,
    increment_dom: Increment<u8>,
    increment_month: Increment<u8>,
    dow: Option<Weekday>,
}

impl Schedule {
    pub fn next(&mut self) -> DateTime<Local> {
        loop {
            self.increment_date();
            let candidate = Local.ymd_opt(
                self.current.year,
                self.current.month as u32,
                self.current.day as u32,
            );

            // FIXME: the not-earlier-than time filter is probably ineffective, because it's
            // only testing the date, not the hours/minutes/seconds.
            if let LocalResult::Single(candidate) = candidate {
                if candidate >= Local::today() && self.is_valid_weekday(&candidate) {
                    return candidate.and_hms(
                        self.current.hour as u32,
                        self.current.minute as u32,
                        0,
                    );
                }
            }
        }
    }

    fn increment_date(&mut self) {
        if !self.increment_minute() {
            return;
        }

        if !self.increment_hour() {
            return;
        }

        if !self.increment_dom() {
            return;
        }

        if !self.increment_month() {
            return;
        }

        self.current.year += 1;
    }

    fn increment_minute(&mut self) -> bool {
        let (next, must_increment) = self.increment_minute.next().unwrap();
        self.current.minute = next;
        must_increment
    }

    fn increment_hour(&mut self) -> bool {
        let (next, must_increment) = self.increment_hour.next().unwrap();
        self.current.hour = next;
        must_increment
    }

    fn increment_dom(&mut self) -> bool {
        let (next, must_increment) = self.increment_dom.next().unwrap();
        self.current.day = next;
        must_increment
    }

    fn increment_month(&mut self) -> bool {
        let (next, must_increment) = self.increment_month.next().unwrap();
        self.current.month = next;
        must_increment
    }

    fn is_valid_weekday<Z: TimeZone>(&self, date: &Date<Z>) -> bool {
        self.dow
            .as_ref()
            .map(|dow| dow.is_valid(date))
            .unwrap_or(true)
    }
}

/// Represents a datetime-like value which may or may not be a valid datetime.
struct CandidateDateTime {
    minute: u8,
    hour: u8,
    day: u8,
    month: u8,
    year: i32,
}

enum Increment<T> {
    Single(T),
    Multiple(SetTicker<T>),
    Range(RangeTicker<T>),
}

impl<T: Copy + Ticker> Iterator for Increment<T> {
    type Item = (T, bool);

    fn next(&mut self) -> Option<Self::Item> {
        match *self {
            Increment::Single(x) => Some((x, true)),
            Increment::Multiple(ref mut x) => x.next(),
            Increment::Range(ref mut x) => x.next(),
        }
    }
}

struct SetTicker<T> {
    idx: usize,
    set: Vec<T>,
}

impl<T: Copy> SetTicker<T> {
    fn new(set: Vec<T>) -> SetTicker<T> {
        SetTicker { idx: 0, set }
    }
}

impl<T: Copy> Iterator for SetTicker<T> {
    type Item = (T, bool);

    fn next(&mut self) -> Option<Self::Item> {
        match self.idx {
            0 => self.set.get(0).map(|&x| (x, false)),
            idx => {
                self.idx += 1;
                match self.set.get(idx) {
                    None => {
                        self.idx = 1;
                        self.set.get(0).map(|&x| (x, true))
                    }

                    some => some.map(|&x| (x, false)),
                }
            }
        }
    }
}

struct RangeTicker<T> {
    min: T,
    max: T,
    current: T,
}

impl<T: Copy> RangeTicker<T> {
    fn new(min: T, max: T) -> RangeTicker<T> {
        RangeTicker {
            min,
            max,
            current: min,
        }
    }

    fn with_current(min: T, max: T, current: T) -> RangeTicker<T> {
        RangeTicker { min, max, current }
    }
}

impl<T: Copy + Ticker> Iterator for RangeTicker<T> {
    type Item = (T, bool);

    fn next(&mut self) -> Option<Self::Item> {
        match self.current {
            current if current >= self.min && current <= self.max => {
                self.current = self.current.increment();
                Some((current, false))
            }

            _ => {
                self.current = self.min.increment();
                Some((self.min, true))
            }
        }
    }
}
