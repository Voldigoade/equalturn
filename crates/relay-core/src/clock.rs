use std::time::{SystemTime, UNIX_EPOCH};

pub fn unix_time_ms() -> Result<u64, std::time::SystemTimeError> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH)?;
    Ok(u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
}
