use chrono::{DateTime, Utc};
use chrono_tz::{Asia::Tokyo, Tz};

///
/// get the difference between the current time and the given date
/// ex. 50s, 5m, 2h, 1d, 10M, 2y, 15y,
pub fn get_diff(date: DateTime<Tz>) -> String {
    let now = Utc::now().with_timezone(&Tokyo);
    let duration = now - date;
    let seconds = duration.num_seconds();

    let (value, unit) = if seconds >= 31536000 {
        (seconds / 31536000, "y")
    } else if seconds >= 2592000 {
        (seconds / 2592000, "M")
    } else if seconds >= 86400 {
        (seconds / 86400, "d")
    } else if seconds >= 3600 {
        (seconds / 3600, "h")
    } else if seconds >= 60 {
        (seconds / 60, "m")
    } else {
        (seconds, "s")
    };

    format!("{value}{unit}")
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_diff() {
        let now = Utc::now().with_timezone(&Tokyo);
        let one_minute_ago = now - chrono::Duration::minutes(1);
        let five_minutes_ago = now - chrono::Duration::minutes(5);
        let one_hour_ago = now - chrono::Duration::hours(1);
        let one_day_ago = now - chrono::Duration::days(1);
        let one_month_ago = now - chrono::Duration::days(30);
        let one_year_ago = now - chrono::Duration::days(365);

        assert_eq!(get_diff(one_minute_ago), "1m");
        assert_eq!(get_diff(five_minutes_ago), "5m");
        assert_eq!(get_diff(one_hour_ago), "1h");
        assert_eq!(get_diff(one_day_ago), "1d");
        assert_eq!(get_diff(one_month_ago), "1M");
        assert_eq!(get_diff(one_year_ago), "1y");
    }
}
