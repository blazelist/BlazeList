//! Due date utilities shared across BlazeList clients.
//!
//! Provides due-date status computation, badge formatting, and quick-pick
//! presets that are platform-agnostic.

use chrono::{DateTime, Datelike, Utc, Weekday};

/// Due date status relative to today.
pub enum DueDateStatus {
    /// Due date is in the past by this many days.
    Overdue(i64),
    /// Due date is today.
    Today,
    /// Due date is in the future by this many days.
    Upcoming(i64),
}

/// Compute the due date status relative to today (UTC).
pub fn due_date_status(due_date: &DateTime<Utc>) -> DueDateStatus {
    let today = Utc::now().date_naive();
    let due = due_date.date_naive();
    let diff = due.signed_duration_since(today).num_days();
    if diff < 0 {
        DueDateStatus::Overdue(-diff)
    } else if diff == 0 {
        DueDateStatus::Today
    } else {
        DueDateStatus::Upcoming(diff)
    }
}

/// Format a due date as a badge string and CSS class.
///
/// Returns `(text, css_class)`.
pub fn format_due_date_badge(due_date: &DateTime<Utc>) -> (String, &'static str) {
    match due_date_status(due_date) {
        DueDateStatus::Overdue(days) => {
            if days == 1 {
                ("1d overdue".into(), "due-overdue")
            } else {
                (format!("{days}d overdue"), "due-overdue")
            }
        }
        DueDateStatus::Today => ("due today".into(), "due-today"),
        DueDateStatus::Upcoming(days) => {
            if days == 1 {
                ("tomorrow".into(), "due-upcoming")
            } else {
                (format!("in {days}d"), "due-upcoming")
            }
        }
    }
}

/// Format a due date for display in detail metadata.
pub fn format_due_date_display(due_date: &DateTime<Utc>) -> String {
    let date_str = due_date.format("%Y-%m-%d").to_string();
    let (badge, _) = format_due_date_badge(due_date);
    format!("{date_str} ({badge})")
}

/// Quick-pick preset for due dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DueDatePreset {
    Today,
    Tomorrow,
    InTwoDays,
    NextMon,
    NextFri,
}

impl DueDatePreset {
    pub const ALL: [Self; 5] = [Self::Today, Self::Tomorrow, Self::InTwoDays, Self::NextMon, Self::NextFri];

    pub fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Tomorrow => "Tomorrow",
            Self::InTwoDays => "In 2 Days",
            Self::NextMon => "Next Mon",
            Self::NextFri => "Next Fri",
        }
    }

    pub fn resolve(self) -> DateTime<Utc> {
        match self {
            Self::Today => today_midnight(),
            Self::Tomorrow => tomorrow_midnight(),
            Self::InTwoDays => in_two_days_midnight(),
            Self::NextMon => next_weekday(Weekday::Mon),
            Self::NextFri => next_weekday(Weekday::Fri),
        }
    }
}

/// Today at midnight UTC.
pub fn today_midnight() -> DateTime<Utc> {
    Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
}

/// Tomorrow at midnight UTC.
pub fn tomorrow_midnight() -> DateTime<Utc> {
    let tomorrow = Utc::now().date_naive().succ_opt().unwrap();
    tomorrow.and_hms_opt(0, 0, 0).unwrap().and_utc()
}

/// Two days from now at midnight UTC.
pub fn in_two_days_midnight() -> DateTime<Utc> {
    let two_days = Utc::now().date_naive() + chrono::Duration::days(2);
    two_days.and_hms_opt(0, 0, 0).unwrap().and_utc()
}

/// The next occurrence of the given weekday at midnight UTC.
///
/// If today is the target weekday, returns the same day next week.
pub fn next_weekday(weekday: Weekday) -> DateTime<Utc> {
    let today = Utc::now().date_naive();
    let today_wd = today.weekday().num_days_from_monday();
    let target_wd = weekday.num_days_from_monday();
    let days_ahead = ((target_wd as i32 - today_wd as i32) + 7) % 7;
    let days_ahead = if days_ahead == 0 { 7 } else { days_ahead };
    let date = today + chrono::Duration::days(i64::from(days_ahead));
    date.and_hms_opt(0, 0, 0).unwrap().and_utc()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_date_status_overdue() {
        let yesterday = Utc::now() - chrono::Duration::days(2);
        match due_date_status(&yesterday) {
            DueDateStatus::Overdue(days) => assert!(days >= 1),
            other => panic!("expected Overdue, got {:?}", std::mem::discriminant(&other)),
        }
    }

    #[test]
    fn due_date_status_today() {
        let today = Utc::now()
            .date_naive()
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc();
        assert!(matches!(due_date_status(&today), DueDateStatus::Today));
    }

    #[test]
    fn due_date_status_upcoming() {
        let future = Utc::now() + chrono::Duration::days(5);
        match due_date_status(&future) {
            DueDateStatus::Upcoming(days) => assert!(days >= 4),
            other => panic!(
                "expected Upcoming, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    fn format_badge_overdue() {
        let past = Utc::now() - chrono::Duration::days(3);
        let (text, class) = format_due_date_badge(&past);
        assert!(text.contains("overdue"), "text={text}");
        assert_eq!(class, "due-overdue");
    }

    #[test]
    fn format_badge_today() {
        let today = Utc::now()
            .date_naive()
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc();
        let (text, class) = format_due_date_badge(&today);
        assert_eq!(text, "due today");
        assert_eq!(class, "due-today");
    }

    #[test]
    fn format_badge_tomorrow() {
        let tomorrow = Utc::now()
            .date_naive()
            .succ_opt()
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc();
        let (text, class) = format_due_date_badge(&tomorrow);
        assert_eq!(text, "tomorrow");
        assert_eq!(class, "due-upcoming");
    }

    #[test]
    fn format_display_includes_date_and_badge() {
        let today = Utc::now()
            .date_naive()
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc();
        let display = format_due_date_display(&today);
        assert!(display.contains("due today"), "display={display}");
        // Should contain the date in YYYY-MM-DD format
        let date_str = today.format("%Y-%m-%d").to_string();
        assert!(display.contains(&date_str), "display={display}");
    }

    #[test]
    fn today_midnight_is_midnight() {
        let t = today_midnight();
        assert_eq!(t.time(), chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    }

    #[test]
    fn tomorrow_midnight_is_after_today() {
        let today = today_midnight();
        let tomorrow = tomorrow_midnight();
        assert_eq!(
            (tomorrow - today).num_days(),
            1,
            "tomorrow should be exactly 1 day after today"
        );
    }

    #[test]
    fn in_two_days_midnight_is_after_tomorrow() {
        let today = today_midnight();
        let two_days = in_two_days_midnight();
        assert_eq!(
            (two_days - today).num_days(),
            2,
            "in_two_days should be exactly 2 days after today"
        );
        assert_eq!(
            two_days.time(),
            chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            "in_two_days should resolve to midnight"
        );
    }

    #[test]
    fn next_weekday_never_today() {
        let today_wd = Utc::now().date_naive().weekday();
        let result = next_weekday(today_wd);
        assert!(
            result.date_naive() > Utc::now().date_naive(),
            "next_weekday should return a future date"
        );
        assert_eq!(
            (result.date_naive() - Utc::now().date_naive()).num_days(),
            7,
            "same weekday should be 7 days away"
        );
    }

    #[test]
    fn preset_labels_unique() {
        let labels: Vec<_> = DueDatePreset::ALL.iter().map(|p| p.label()).collect();
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(unique.len(), labels.len(), "duplicate preset labels");
    }

    #[test]
    fn preset_resolve_returns_midnight() {
        for preset in DueDatePreset::ALL {
            let dt = preset.resolve();
            assert_eq!(
                dt.time(),
                chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                "{:?} should resolve to midnight",
                preset,
            );
        }
    }
}
