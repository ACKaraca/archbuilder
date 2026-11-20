//! Time stamps for the header of an export: the caller decides which instant to write.

/// The ISO 8601 date and time (`2026-01-02T03:04:05`, UTC, no zone suffix) of `unix_millis`.
#[must_use]
pub fn iso8601(unix_millis: u64) -> String {
    let seconds = unix_millis / 1000;
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Days since 1970-01-01 to a civil date (proleptic Gregorian, shifted to start in March).
    let z = i64::try_from(days).unwrap_or(0) + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let m = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * m + 2) / 5 + 1;
    let month = if m < 10 { m + 3 } else { m - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    use super::iso8601;

    #[test]
    fn formats_known_instants() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00");
        assert_eq!(iso8601(951_827_696_000), "2000-02-29T12:34:56");
        assert_eq!(iso8601(1_798_761_599_999), "2026-12-31T23:59:59");
    }
}
