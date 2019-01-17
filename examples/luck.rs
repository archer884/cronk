use chrono::{DateTime, Local};
use cron::Schedule as CronSchedule;
use cronk::{Expression, Field, Schedule, Weekday};
use stopwatch::Stopwatch;

macro_rules! time {
    ($item:expr) => {{
        let mut time = Stopwatch::start_new();
        let result = $item;
        time.stop();
        (result, time.elapsed())
    }};
}

fn main() {
    let expression = Expression {
        minute: Some(Field::Single(0)),
        hour: Some(Field::Single(17)),
        dom: Some(Field::Single(13)),
        month: None,
        dow: Some(Weekday {
            field: Field::Single(5),
            nth: None,
        }),
    };

    let (cronk_next_hundred, cronk_elapsed) = time!(ScheduleIterator(expression.into_schedule())
        .take(100)
        .collect::<Vec<_>>());

    let other_schedule = "0 0 17 13 * Fri".parse::<CronSchedule>().unwrap();
    let (other_next_hundred, other_elapsed) =
        time!(other_schedule.upcoming(Local).take(100).collect::<Vec<_>>());

    println!("Cronk: {:?}", cronk_elapsed);
    println!("Other: {:?}", other_elapsed);

    for (left, right) in cronk_next_hundred
        .into_iter()
        .zip(other_next_hundred.into_iter())
    {
        println!("{} :: {}", left, right);
        assert_eq!(left, right);
    }
}

struct ScheduleIterator(Schedule);

impl Iterator for ScheduleIterator {
    type Item = DateTime<Local>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.0.next())
    }
}
