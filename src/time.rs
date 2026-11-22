use std::time::Duration;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ParseError {
    #[error("no entiendo {0:?} · usa 45s, 90m, 2h, 1h30m")]
    Unknown(String),
    #[error("el tiempo debe ser mayor que 0")]
    Zero,
    #[error("{0:?} es demasiado largo · máximo 365d, o usa inf")]
    TooLong(String),
}

const MAX_SECS: f64 = 365.0 * 86400.0;

fn to_duration(secs: f64, text: &str) -> Result<Option<Duration>, ParseError> {
    if secs.is_nan() {
        return Err(ParseError::Unknown(text.into()));
    }
    if secs <= 0.0 {
        return Err(ParseError::Zero);
    }
    if secs > MAX_SECS {
        return Err(ParseError::TooLong(text.into()));
    }
    Ok(Some(Duration::from_secs_f64(secs)))
}

/// `"45s"`, `"90m"`, `"2h"`, `"1h30m"`, `"90"` (=min), `"inf"` → `None`.
pub fn parse_duration(text: &str) -> Result<Option<Duration>, ParseError> {
    let t = text.trim().to_lowercase().replace(',', ".");
    if matches!(
        t.as_str(),
        "" | "0" | "inf" | "infinite" | "infinity" | "forever" | "siempre"
    ) {
        return Ok(None);
    }
    // Solo dígitos y punto: evita que f64 acepte "nan", "+inf" o "1e9".
    if t.chars().all(|c| c.is_ascii_digit() || c == '.') {
        let n: f64 = t.parse().map_err(|_| ParseError::Unknown(text.into()))?;
        return to_duration(n * 60.0, text);
    }

    let (mut total, mut num, mut matched) = (0.0f64, String::new(), false);
    for c in t.chars() {
        match c {
            '0'..='9' | '.' => num.push(c),
            's' | 'm' | 'h' | 'd' => {
                let v: f64 = num.parse().map_err(|_| ParseError::Unknown(text.into()))?;
                total += v * match c {
                    's' => 1.0,
                    'm' => 60.0,
                    'h' => 3600.0,
                    _ => 86400.0,
                };
                num.clear();
                matched = true;
            }
            ' ' => {}
            _ => return Err(ParseError::Unknown(text.into())),
        }
    }
    if !matched || !num.is_empty() {
        return Err(ParseError::Unknown(text.into()));
    }
    to_duration(total, text)
}

pub fn fmt_hms(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

pub fn fmt_short(d: Option<Duration>) -> String {
    let Some(d) = d else {
        return "sin límite".into();
    };
    let s = d.as_secs();
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    let mut out = Vec::new();
    if h > 0 {
        out.push(format!("{h}h"));
    }
    if m > 0 {
        out.push(format!("{m}m"));
    }
    if sec > 0 || out.is_empty() {
        out.push(format!("{sec}s"));
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn secs(t: &str) -> u64 {
        parse_duration(t).unwrap().unwrap().as_secs()
    }

    #[test]
    fn formatos() {
        assert_eq!(secs("45s"), 45);
        assert_eq!(secs("90m"), 5400);
        assert_eq!(secs("2h"), 7200);
        assert_eq!(secs("1h30m"), 5400);
        assert_eq!(secs("90"), 5400); // número puro = minutos
        assert_eq!(secs("1d"), 86400);
        assert_eq!(secs("1,5h"), 5400); // coma decimal
    }

    #[test]
    fn infinitos() {
        for t in ["", "0", "inf", "infinity", "forever", "siempre", "  INF  "] {
            assert_eq!(parse_duration(t).unwrap(), None, "falló con {t:?}");
        }
    }

    #[test]
    fn basura() {
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("1h30").is_err()); // cola sin unidad
        assert!(parse_duration("-5").is_err());
        assert_eq!(parse_duration("0s"), Err(ParseError::Zero));
    }

    #[test]
    fn no_entra_en_panico() {
        for t in [
            "nan",
            "+inf",
            "-inf",
            "1e300",
            "1e3",
            "99999999999999999999h",
            "..",
            "5.5.5m",
        ] {
            assert!(parse_duration(t).is_err(), "debió fallar con {t:?}");
        }
        assert!(matches!(
            parse_duration("366d"),
            Err(ParseError::TooLong(_))
        ));
        assert_eq!(secs("365d"), 365 * 86400);
    }

    #[test]
    fn formateo() {
        assert_eq!(fmt_hms(Duration::from_secs(5400)), "01:30:00");
        assert_eq!(fmt_short(Some(Duration::from_secs(5400))), "1h 30m");
        assert_eq!(fmt_short(Some(Duration::from_secs(45))), "45s");
        assert_eq!(fmt_short(None), "sin límite");
    }
}
