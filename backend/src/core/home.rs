use crate::error::BrainError;
use chrono::{Local, NaiveDate};

/// Browser calendar day; parsing must round-trip to reject ambiguous dates.
pub fn overview_date(value: Option<&str>) -> Result<NaiveDate, BrainError> {
    let Some(value) = value else {
        return Ok(Local::now().date_naive());
    };
    if value.len() != 10
        || !value.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
    {
        return Err(BrainError::TaskValidation("首页日期应为 YYYY-MM-DD".into()));
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .ok()
        .filter(|day| day.to_string() == value)
        .ok_or_else(|| BrainError::TaskValidation("首页日期应为 YYYY-MM-DD".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_overview_date_rejects_invalid_and_ambiguous_dates() {
        assert!(overview_date(Some("2026-10-04")).is_ok());
        for day in [
            "2026-2-4",
            "2026-02-30",
            "invalid",
            "2026-10-04 OR 1=1",
            "-262143-01-01",
        ] {
            assert!(overview_date(Some(day)).is_err());
        }
        assert!(overview_date(None).is_ok());
    }
}
