//! Small formatting helpers.

/// `1234567` → `"1,234,567"`.
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// `"1 item"`, `"2 items"`.
pub fn count(n: usize, noun: &str) -> String {
    format!(
        "{} {noun}{}",
        thousands(n as u64),
        if n == 1 { "" } else { "s" }
    )
}

/// Civil date and time of a Unix timestamp: (year, month, day, hour, minute, second).
pub fn civil(ts: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = ts.div_euclid(86_400);
    let secs = ts.rem_euclid(86_400);
    // Howard Hinnant's days_from_civil inverse
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (
        y,
        m,
        d,
        (secs / 3600) as u32,
        (secs % 3600 / 60) as u32,
        (secs % 60) as u32,
    )
}

/// `"2026-11-14 09.05.33"` (safe in file names).
pub fn file_stamp(ts: i64) -> String {
    let (y, mo, d, h, mi, s) = civil(ts);
    format!("{y:04}-{mo:02}-{d:02} {h:02}.{mi:02}.{s:02}")
}

/// `"2026-11-14 09:05"`.
pub fn display_stamp(ts: i64) -> String {
    let (y, mo, d, h, mi, _) = civil(ts);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}")
}

/// `"12.3 MB"`.
pub fn size(bytes: u64) -> String {
    if bytes >= 1 << 20 {
        format!("{:.1} MB", bytes as f64 / (1 << 20) as f64)
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(file_stamp(0), "1970-01-01 00.00.00");
        assert_eq!(display_stamp(1_763_112_333), "2025-11-14 09:25");
        assert_eq!(civil(951_782_400).1, 2, "2000-02-29");
        assert_eq!(civil(951_782_400).2, 29);
    }
}
