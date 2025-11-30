use anyhow::{Result, anyhow};
use chrono::{DateTime, Duration, TimeZone, Utc};
use chrono_tz::EST;

/// Return the release time of the puzzle for the given year and day
pub fn release_time(year: i32, day: u32) -> Result<DateTime<Utc>> {
    if day == 0 || day > 25 {
        return Err(anyhow!("Day must be between 1 and 25"));
    }
    // Unwrap is OK here since we know that no time change should happen in december
    Ok(EST
        .with_ymd_and_hms(year, 12, day, 0, 0, 0)
        .single()
        .unwrap()
        .with_timezone(&Utc))
}

/// Calculate the score for a puzzle based on the duration from release
pub fn score_puzzle(completion_time: Duration) -> usize {
    const NUM_DAYS_WITHOUT_PENALTY: usize = 7;
    const MAX_SCORE: usize = 50;
    const MIN_SCORE: usize = 10;
    const PENALTY_PER_DAY: usize = 5;
    // After the `NUM_DAYS_WITHOUT_PENALTY` number of days has passed,
    // the score will be decreased by `PENALTY_PER_DAY` points per day.
    let num_days = completion_time.num_days() as usize;
    if num_days <= NUM_DAYS_WITHOUT_PENALTY {
        MAX_SCORE
    } else {
        let penalty_days = num_days - NUM_DAYS_WITHOUT_PENALTY;
        let penalty = penalty_days * PENALTY_PER_DAY;
        let score = MAX_SCORE.saturating_sub(penalty);
        score.max(MIN_SCORE)
    }
}
