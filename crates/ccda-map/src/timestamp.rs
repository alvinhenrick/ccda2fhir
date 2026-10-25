//! HL7 v3 timestamps (`TS`) to FHIR `date` and `dateTime`.
//!
//! The grammar is `YYYY[MM[DD[HH[MM[SS[.S+]]]]]][+/-ZZ[zz]]`. Rules follow the IG's "CDA <-> FHIR
//! Time/Dates" guidance: keep the precision the source has, preserve the offset, and never invent
//! one.

use crate::Report;

/// What was lost or assumed while converting a timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loss {
    /// The value had a time but no offset. FHIR requires an offset once a time is present, so the
    /// time was dropped and only the date was kept. (The IG lists this as one valid approach.)
    TimeDroppedNoOffset,
    /// The value had no seconds (or no minutes). FHIR R4 `dateTime` requires `hh:mm:ss`, so the
    /// missing parts were filled with zeros.
    PrecisionExtended,
}

/// A successfully converted timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Converted {
    /// The FHIR `dateTime` string.
    pub value: String,
    /// Set when the conversion was lossy or assumed something.
    pub loss: Option<Loss>,
}

/// The value is not a valid HL7 timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTimestamp;

/// The pieces of a validated timestamp. All slices are ASCII digits.
struct Parts<'a> {
    year: &'a str,
    month: Option<&'a str>,
    day: Option<&'a str>,
    hour: Option<&'a str>,
    minute: Option<&'a str>,
    second: Option<&'a str>,
    fraction: Option<&'a str>,
    /// Sign, hours, minutes of the offset.
    offset: Option<(char, &'a str, &'a str)>,
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn num(s: &str) -> u32 {
    s.parse().unwrap_or(u32::MAX)
}

fn is_leap(year: u32) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        _ => 28,
    }
}

fn parse(value: &str) -> Option<Parts<'_>> {
    let (main, offset_text) = match value.find(['+', '-']) {
        Some(i) => (&value[..i], Some(&value[i..])),
        None => (value, None),
    };
    let (digits, fraction) = match main.split_once('.') {
        Some((d, f)) => (d, Some(f)),
        None => (main, None),
    };
    if !all_digits(digits) || !matches!(digits.len(), 4 | 6 | 8 | 10 | 12 | 14) {
        return None;
    }
    // A fraction only makes sense after seconds.
    if let Some(f) = fraction {
        if digits.len() != 14 || !all_digits(f) {
            return None;
        }
    }

    let year = digits.get(0..4)?;
    let month = digits.get(4..6);
    let day = digits.get(6..8);
    let hour = digits.get(8..10);
    let minute = digits.get(10..12);
    let second = digits.get(12..14);

    let y = num(year);
    if let Some(m) = month {
        if !(1..=12).contains(&num(m)) {
            return None;
        }
    }
    if let (Some(m), Some(d)) = (month, day) {
        if !(1..=days_in_month(y, num(m))).contains(&num(d)) {
            return None;
        }
    }
    if hour.is_some_and(|h| num(h) > 23)
        || minute.is_some_and(|m| num(m) > 59)
        || second.is_some_and(|s| num(s) > 60)
    {
        return None;
    }

    let offset = match offset_text {
        None => None,
        Some(text) => {
            let sign = text.chars().next()?;
            let rest = text.get(1..)?;
            if !all_digits(rest) || !matches!(rest.len(), 2 | 4) {
                return None;
            }
            let hh = rest.get(0..2)?;
            let mm = rest.get(2..4).unwrap_or("00");
            let (h, m) = (num(hh), num(mm));
            if h > 14 || m > 59 || (h == 14 && m != 0) {
                return None;
            }
            Some((sign, hh, mm))
        }
    };

    Some(Parts {
        year,
        month,
        day,
        hour,
        minute,
        second,
        fraction,
        offset,
    })
}

fn date_string(p: &Parts) -> String {
    match (p.month, p.day) {
        (Some(m), Some(d)) => format!("{}-{m}-{d}", p.year),
        (Some(m), None) => format!("{}-{m}", p.year),
        _ => p.year.to_owned(),
    }
}

/// Convert an HL7 `TS` value to a FHIR `date`, keeping the precision it has.
///
/// `19700101120000-0500` becomes `1970-01-01`, `197001` becomes `1970-01`, `1970` becomes `1970`.
/// Returns `None` if the value is not a valid HL7 timestamp (wrong length, non-digits, impossible
/// month or day, bad offset). The time and offset are dropped on purpose: a `date` has neither.
#[must_use]
pub fn hl7_date_to_fhir_date(value: &str) -> Option<String> {
    parse(value).map(|p| date_string(&p))
}

/// Convert an HL7 `TS` value to a FHIR `dateTime`.
///
/// # Errors
///
/// Returns [`InvalidTimestamp`] if the value is not a valid HL7 timestamp.
pub fn hl7_ts_to_fhir_datetime(value: &str) -> Result<Converted, InvalidTimestamp> {
    let p = parse(value).ok_or(InvalidTimestamp)?;
    let date = date_string(&p);

    let (Some(hour), Some((sign, off_h, off_m))) = (p.hour, p.offset) else {
        // Date only (offset on a date is meaningless), or a time with no offset.
        let loss = p.hour.map(|_| Loss::TimeDroppedNoOffset);
        return Ok(Converted { value: date, loss });
    };

    let extended = p.minute.is_none() || p.second.is_none();
    let minute = p.minute.unwrap_or("00");
    let second = p.second.unwrap_or("00");
    let fraction = p.fraction.map(|f| format!(".{f}")).unwrap_or_default();
    Ok(Converted {
        value: format!("{date}T{hour}:{minute}:{second}{fraction}{sign}{off_h}:{off_m}"),
        loss: extended.then_some(Loss::PrecisionExtended),
    })
}

/// Convert a timestamp and record anything lossy in the report.
///
/// Returns `None` (with a warning) if the value is invalid.
pub fn map_datetime(value: &str, report: &mut Report) -> Option<String> {
    match hl7_ts_to_fhir_datetime(value) {
        Ok(converted) => {
            match converted.loss {
                Some(Loss::TimeDroppedNoOffset) => report.warn(
                    "timestamp.time-dropped.no-offset",
                    "a timestamp had a time but no time zone offset; FHIR requires an offset, so only the date was kept",
                ),
                Some(Loss::PrecisionExtended) => report.info(
                    "timestamp.precision-extended",
                    "a timestamp had no seconds (or minutes); FHIR requires them, so zeros were filled in",
                ),
                None => {}
            }
            Some(converted.value)
        }
        Err(InvalidTimestamp) => {
            report.warn(
                "timestamp.invalid",
                "a value is not a valid HL7 timestamp and was omitted",
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(value: &str) -> Converted {
        hl7_ts_to_fhir_datetime(value).unwrap()
    }

    #[test]
    fn date_keeps_available_precision() {
        assert_eq!(hl7_date_to_fhir_date("1970").as_deref(), Some("1970"));
        assert_eq!(hl7_date_to_fhir_date("197001").as_deref(), Some("1970-01"));
        assert_eq!(
            hl7_date_to_fhir_date("19700102").as_deref(),
            Some("1970-01-02")
        );
    }

    #[test]
    fn date_drops_time_and_offset() {
        assert_eq!(
            hl7_date_to_fhir_date("19700102120000-0500").as_deref(),
            Some("1970-01-02")
        );
    }

    #[test]
    fn rejects_bad_values() {
        for bad in [
            "",
            "19",
            "abcd",
            "19701301",
            "19700132",
            "19700230",
            "197",
            "19700",
            "1970-01-02",
            "19700102T1200",
            "19700102120000+5",
            "19700102120000+1500",
            "1970.5",
            "19700102246000-0500",
            "19700102120060-0500x",
        ] {
            assert_eq!(hl7_date_to_fhir_date(bad), None, "{bad}");
            assert_eq!(hl7_ts_to_fhir_datetime(bad), Err(InvalidTimestamp), "{bad}");
        }
    }

    #[test]
    fn validates_leap_days() {
        assert!(hl7_date_to_fhir_date("20240229").is_some());
        assert!(hl7_date_to_fhir_date("20230229").is_none());
        assert!(hl7_date_to_fhir_date("19000229").is_none());
        assert!(hl7_date_to_fhir_date("20000229").is_some());
    }

    #[test]
    fn datetime_date_only_has_no_loss() {
        assert_eq!(
            dt("20230531"),
            Converted {
                value: "2023-05-31".into(),
                loss: None
            }
        );
        assert_eq!(dt("202305").value, "2023-05");
        assert_eq!(dt("2023").value, "2023");
    }

    #[test]
    fn datetime_preserves_offset_and_seconds() {
        assert_eq!(
            dt("20230531220500-0500"),
            Converted {
                value: "2023-05-31T22:05:00-05:00".into(),
                loss: None
            }
        );
        assert_eq!(dt("20230531220500+0530").value, "2023-05-31T22:05:00+05:30");
        assert_eq!(dt("20230531220500+00").value, "2023-05-31T22:05:00+00:00");
    }

    #[test]
    fn datetime_keeps_fractional_seconds() {
        assert_eq!(
            dt("20230531220500.123-0500").value,
            "2023-05-31T22:05:00.123-05:00"
        );
    }

    #[test]
    fn datetime_fills_missing_seconds_and_says_so() {
        // The IG's example shows T22:05-05:00, but FHIR R4 dateTime requires seconds.
        assert_eq!(
            dt("202305312205-0500"),
            Converted {
                value: "2023-05-31T22:05:00-05:00".into(),
                loss: Some(Loss::PrecisionExtended)
            }
        );
        assert_eq!(dt("2023053122-0500").value, "2023-05-31T22:00:00-05:00");
    }

    #[test]
    fn datetime_without_offset_drops_the_time() {
        assert_eq!(
            dt("20230531220500"),
            Converted {
                value: "2023-05-31".into(),
                loss: Some(Loss::TimeDroppedNoOffset)
            }
        );
    }

    #[test]
    fn map_datetime_reports_loss_without_values() {
        let mut report = Report::default();
        assert_eq!(
            map_datetime("20230531220500", &mut report).as_deref(),
            Some("2023-05-31")
        );
        assert_eq!(
            map_datetime("20230531", &mut report).as_deref(),
            Some("2023-05-31")
        );
        assert_eq!(map_datetime("nonsense", &mut report), None);
        let codes: Vec<_> = report.issues.iter().map(|i| i.code).collect();
        assert_eq!(
            codes,
            ["timestamp.time-dropped.no-offset", "timestamp.invalid"]
        );
        assert!(report.issues.iter().all(|i| !i.message.contains("2023")));
    }
}
