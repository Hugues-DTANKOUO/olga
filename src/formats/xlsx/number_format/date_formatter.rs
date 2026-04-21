//! POI-aligned CellDateFormatter + CellElapsedFormatter — renders a
//! Date- or Elapsed-kind section body against a cell serial.
//!
//! # Responsibilities
//!
//! Two entry points consume bodies already kind-tagged by the section
//! splitter (#39):
//!
//! * [`format_date_body`] — calendrical rendering. The serial is
//!   decomposed into `(year, month, day, hour, minute, second, frac_s,
//!   weekday)` using Julian Day Number arithmetic, with Excel's Lotus
//!   1900-leap-year bug honoured in the 1900 epoch path and the 1904
//!   epoch supported via the `date_1904` argument.
//! * [`format_elapsed_body`] — duration rendering for bodies with
//!   `[h]`/`[m]`/`[s]` markers. The largest elapsed marker consumes the
//!   serial as a total count of its own unit, and non-bracketed hour /
//!   minute / second tokens render the modular remainders so a format
//!   like `[h]:mm:ss` on 1.5 days renders `36:00:00`.
//!
//! Both paths share tokenisation, the ambiguous-`m` resolver, and the
//! fractional-second rounding rule (precision driven by the number of
//! `0`s after the last `s`/`ss` token).
//!
//! # Ambiguous `m` resolution
//!
//! Excel's grammar overloads `m` for month and minute. POI's rule —
//! replicated here — is positional: a `m` / `mm` run is a **minute**
//! iff the nearest non-literal predecessor is an hour token (`h`,
//! `hh`, or a bracketed `[h]`) **or** the nearest non-literal successor
//! is a second token (`s`, `ss`, bracketed `[s]`) or an AM/PM marker.
//! Otherwise the run is a **month**.
//!
//! # Lotus 1900-leap-year bug
//!
//! Excel's 1900 date system (the default) pretends 1900 was a leap year
//! to stay compatible with Lotus 1-2-3. Serial `60` is displayed as
//! `1900-02-29`, a date that doesn't exist. Beyond serial 60, every
//! date is shifted by one day relative to a clean Gregorian calendar.
//! The JDN formula is therefore split into three ranges:
//!
//! * `serial < 60`  → `JDN = 2415020 + serial` (Jan 1 1900 through Feb 28 1900).
//! * `serial == 60` → fake date `(1900, 2, 29)`; weekday computed from
//!   the "real" JDN of that integer offset (which lands on Mar 1 1900,
//!   a Thursday) so the rendered weekday stays monotonic with
//!   surrounding dates.
//! * `serial > 60`  → `JDN = 2415019 + serial` (one-day correction).
//!
//! The 1904 epoch has no such bug: `JDN = 2416481 + serial`.
//!
//! # Fractional-second rounding
//!
//! POI rounds `serial * 86400` to the precision of the first
//! `FracSecond(n)` token (or to the whole second if no such token
//! exists). This guarantees rollover is consistent across minute / hour
//! boundaries — e.g. `ss.000` on a value that rounds to `60.000`
//! increments the minute rather than emitting `60.000` in-place.
//!
//! # Out of scope for this pass (#41)
//!
//! Locale-sensitive month / weekday names (beyond English), era markers
//! (`g`, `gg`, `ggg`), and Era-based year counts (`ee`, `yy/e`). The
//! scaffold renders English names and raises no warnings; locale-aware
//! name tables land with #45's locale pipeline.

use super::contract::{FormatWarning, FormatWarningKind};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Render a Date-kind section body against a cell serial.
///
/// * `value` — Excel serial number (days since the active epoch).
/// * `body` — body text of the chosen `FormatSection`, already stripped
///   of bracket prefixes by the section splitter (#39).
/// * `date_1904` — `true` if the workbook uses the 1904 epoch (Mac
///   Excel default), `false` for the 1900 epoch (Windows Excel
///   default, with Lotus leap-year bug).
///
/// Returns `Err` only when the body fails to tokenise (unterminated
/// quote / bracket / trailing backslash).
pub(super) fn format_date_body(
    value: f64,
    body: &str,
    date_1904: bool,
) -> Result<String, FormatWarning> {
    let raw = tokenize(body)?;
    let tokens = resolve_tokens(&raw);
    let frac_digits = find_frac_digits(&tokens);
    let ctx = DateContext::from_serial(value, date_1904, frac_digits)?;
    let has_am_pm = tokens.iter().any(|t| matches!(t, Token::AmPm(_)));
    let mut out = String::new();
    for tok in &tokens {
        emit_date_token(tok, &ctx, has_am_pm, &mut out);
    }
    Ok(out)
}

/// Render an Elapsed-kind section body against a cell serial.
///
/// The body must contain at least one `[h]` / `[m]` / `[s]` marker;
/// the largest-unit marker consumes the serial as a total count of
/// that unit.
///
/// Returns `Err` only when the body fails to tokenise.
pub(super) fn format_elapsed_body(value: f64, body: &str) -> Result<String, FormatWarning> {
    let raw = tokenize(body)?;
    let tokens = resolve_tokens(&raw);
    let frac_digits = find_frac_digits(&tokens);
    let ctx = ElapsedContext::from_serial(value, frac_digits)?;
    let mut out = String::new();
    for tok in &tokens {
        emit_elapsed_token(tok, &ctx, &mut out);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum AmPmStyle {
    /// `AM/PM` → `"AM"` or `"PM"`.
    UpperFull,
    /// `am/pm` → `"am"` or `"pm"`.
    LowerFull,
    /// `A/P` → `"A"` or `"P"`.
    UpperLetter,
    /// `a/p` → `"a"` or `"p"`.
    LowerLetter,
}

/// First-pass token, produced by [`tokenize`]. `M(n)` is intentionally
/// ambiguous — the [`resolve_tokens`] pass disambiguates month versus
/// minute based on positional neighbours.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RawToken {
    /// Run of `y` characters (1..=4).
    Y(usize),
    /// Run of `m` characters (1..=5, ambiguous).
    M(usize),
    /// Run of `d` characters (1..=4).
    D(usize),
    /// Run of `h` characters (1..=2).
    H(usize),
    /// Run of `s` characters (1..=2).
    S(usize),
    /// `[h]` / `[hh]` elapsed marker (digit count for minimum pad).
    ElapsedH(usize),
    /// `[m]` / `[mm]` elapsed marker.
    ElapsedM(usize),
    /// `[s]` / `[ss]` elapsed marker.
    ElapsedS(usize),
    /// `AM/PM` / `am/pm` / `A/P` / `a/p`.
    AmPm(AmPmStyle),
    /// `.` followed by a run of `0`s, immediately after a seconds
    /// token. Holds the digit count.
    FracSec(usize),
    /// Literal run — quoted, escaped, or intrinsic.
    Literal(String),
    /// `_x` padding directive — renders as a single space (column
    /// width unavailable at extraction time).
    Pad,
    /// `*x` fill directive — dropped.
    Fill,
}

/// Second-pass token after ambiguous-`m` resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Year(usize),
    /// Month as a 1- or 2-digit number.
    MonthNumeric(usize),
    /// `mmm` — short English name.
    MonthShortName,
    /// `mmmm` — long English name.
    MonthLongName,
    /// `mmmmm` — first letter.
    MonthFirstLetter,
    /// Day-of-month as a 1- or 2-digit number.
    Day(usize),
    /// `ddd` — short weekday name.
    WeekdayShort,
    /// `dddd` — long weekday name.
    WeekdayLong,
    Hour(usize),
    Minute(usize),
    Second(usize),
    FracSecond(usize),
    AmPm(AmPmStyle),
    ElapsedHour(usize),
    ElapsedMinute(usize),
    ElapsedSecond(usize),
    Literal(String),
    Pad,
    Fill,
}

// ---------------------------------------------------------------------------
// Tokenisation
// ---------------------------------------------------------------------------

fn tokenize(body: &str) -> Result<Vec<RawToken>, FormatWarning> {
    let chars: Vec<char> = body.chars().collect();
    let n = chars.len();
    let mut i = 0usize;
    let mut out: Vec<RawToken> = Vec::new();

    while i < n {
        let c = chars[i];
        match c {
            '"' => {
                let start = i + 1;
                let mut j = start;
                while j < n && chars[j] != '"' {
                    j += 1;
                }
                if j >= n {
                    return Err(FormatWarning::new(
                        FormatWarningKind::MalformedFormatCode,
                        body,
                        String::new(),
                        "unterminated quoted literal in date body",
                    ));
                }
                let lit: String = chars[start..j].iter().collect();
                push_literal(&mut out, &lit);
                i = j + 1;
            }
            '\\' => {
                if i + 1 >= n {
                    return Err(FormatWarning::new(
                        FormatWarningKind::MalformedFormatCode,
                        body,
                        String::new(),
                        "trailing backslash in date body",
                    ));
                }
                push_literal_char(&mut out, chars[i + 1]);
                i += 2;
            }
            '_' => {
                out.push(RawToken::Pad);
                // Consume the width-reference char (if any).
                i = (i + 2).min(n);
            }
            '*' => {
                out.push(RawToken::Fill);
                i = (i + 2).min(n);
            }
            '[' => {
                let start = i + 1;
                let mut j = start;
                while j < n && chars[j] != ']' {
                    j += 1;
                }
                if j >= n {
                    return Err(FormatWarning::new(
                        FormatWarningKind::MalformedFormatCode,
                        body,
                        String::new(),
                        "unterminated bracket in date body",
                    ));
                }
                let inside: String = chars[start..j].iter().collect();
                let lower = inside.to_ascii_lowercase();
                match lower.as_str() {
                    "h" => out.push(RawToken::ElapsedH(1)),
                    "hh" => out.push(RawToken::ElapsedH(2)),
                    "m" => out.push(RawToken::ElapsedM(1)),
                    "mm" => out.push(RawToken::ElapsedM(2)),
                    "s" => out.push(RawToken::ElapsedS(1)),
                    "ss" => out.push(RawToken::ElapsedS(2)),
                    _ => {
                        // Unknown bracket — treat as literal so quirky
                        // formats don't blow up the stream.
                        push_literal(&mut out, &format!("[{}]", inside));
                    }
                }
                i = j + 1;
            }
            'y' | 'Y' | 'm' | 'M' | 'd' | 'D' | 'h' | 'H' | 's' | 'S' => {
                let lc = c.to_ascii_lowercase();
                let mut count = 0usize;
                while i < n && chars[i].to_ascii_lowercase() == lc {
                    count += 1;
                    i += 1;
                }
                let tok = match lc {
                    'y' => RawToken::Y(count),
                    'm' => RawToken::M(count),
                    'd' => RawToken::D(count),
                    'h' => RawToken::H(count),
                    's' => RawToken::S(count),
                    _ => unreachable!(),
                };
                out.push(tok);
            }
            'A' | 'a' => {
                if let Some((style, consumed)) = try_match_am_pm(&chars, i) {
                    out.push(RawToken::AmPm(style));
                    i += consumed;
                } else {
                    push_literal_char(&mut out, c);
                    i += 1;
                }
            }
            '.' => {
                // Possible start of fractional seconds: `.` followed by
                // a run of `0`s, immediately after a seconds token
                // (plain `s`/`ss` or the elapsed `[s]`/`[ss]` markers).
                let mut zeros = 0usize;
                let mut j = i + 1;
                while j < n && chars[j] == '0' {
                    zeros += 1;
                    j += 1;
                }
                if zeros > 0
                    && matches!(
                        out.last(),
                        Some(RawToken::S(_)) | Some(RawToken::ElapsedS(_))
                    )
                {
                    out.push(RawToken::FracSec(zeros));
                    i = j;
                } else {
                    push_literal_char(&mut out, '.');
                    i += 1;
                }
            }
            _ => {
                push_literal_char(&mut out, c);
                i += 1;
            }
        }
    }
    Ok(out)
}

/// Match an `AM/PM` / `am/pm` / `A/P` / `a/p` token starting at `start`.
/// Case-strict (POI-aligned) — mixed-case variants are not recognised.
fn try_match_am_pm(chars: &[char], start: usize) -> Option<(AmPmStyle, usize)> {
    let rem = &chars[start..];
    // AM/PM (5 chars).
    if rem.len() >= 5
        && rem[0] == 'A'
        && rem[1] == 'M'
        && rem[2] == '/'
        && rem[3] == 'P'
        && rem[4] == 'M'
    {
        return Some((AmPmStyle::UpperFull, 5));
    }
    if rem.len() >= 5
        && rem[0] == 'a'
        && rem[1] == 'm'
        && rem[2] == '/'
        && rem[3] == 'p'
        && rem[4] == 'm'
    {
        return Some((AmPmStyle::LowerFull, 5));
    }
    // A/P (3 chars).
    if rem.len() >= 3 && rem[0] == 'A' && rem[1] == '/' && rem[2] == 'P' {
        return Some((AmPmStyle::UpperLetter, 3));
    }
    if rem.len() >= 3 && rem[0] == 'a' && rem[1] == '/' && rem[2] == 'p' {
        return Some((AmPmStyle::LowerLetter, 3));
    }
    None
}

fn push_literal(out: &mut Vec<RawToken>, lit: &str) {
    if let Some(RawToken::Literal(s)) = out.last_mut() {
        s.push_str(lit);
    } else {
        out.push(RawToken::Literal(lit.to_string()));
    }
}

fn push_literal_char(out: &mut Vec<RawToken>, c: char) {
    if let Some(RawToken::Literal(s)) = out.last_mut() {
        s.push(c);
    } else {
        let mut s = String::new();
        s.push(c);
        out.push(RawToken::Literal(s));
    }
}

// ---------------------------------------------------------------------------
// Resolve — disambiguate M as month-or-minute; shape other tokens
// ---------------------------------------------------------------------------

fn resolve_tokens(raw: &[RawToken]) -> Vec<Token> {
    let mut out = Vec::with_capacity(raw.len());
    for (idx, tok) in raw.iter().enumerate() {
        let resolved = match tok {
            RawToken::Y(n) => Token::Year(*n),
            RawToken::M(n) => {
                if m_is_minute(raw, idx) {
                    // Minute tokens only use 1 or 2 digits of padding;
                    // `mmm`+ in minute position is unusual, but clamp
                    // to 2 so we always emit a valid two-digit pad at
                    // most (POI applies the same "no such thing as mmm
                    // minute" rule).
                    Token::Minute((*n).clamp(1, 2))
                } else {
                    match *n {
                        1 => Token::MonthNumeric(1),
                        2 => Token::MonthNumeric(2),
                        3 => Token::MonthShortName,
                        4 => Token::MonthLongName,
                        _ => Token::MonthFirstLetter,
                    }
                }
            }
            RawToken::D(n) => match *n {
                1 => Token::Day(1),
                2 => Token::Day(2),
                3 => Token::WeekdayShort,
                _ => Token::WeekdayLong,
            },
            RawToken::H(n) => Token::Hour((*n).clamp(1, 2)),
            RawToken::S(n) => Token::Second((*n).clamp(1, 2)),
            RawToken::ElapsedH(n) => Token::ElapsedHour(*n),
            RawToken::ElapsedM(n) => Token::ElapsedMinute(*n),
            RawToken::ElapsedS(n) => Token::ElapsedSecond(*n),
            RawToken::AmPm(s) => Token::AmPm(s.clone()),
            RawToken::FracSec(n) => Token::FracSecond(*n),
            RawToken::Literal(s) => Token::Literal(s.clone()),
            RawToken::Pad => Token::Pad,
            RawToken::Fill => Token::Fill,
        };
        out.push(resolved);
    }
    out
}

/// POI's ambiguous-`m` rule.
///
/// Walk left skipping literals / pad / fill; if the first date-ish
/// predecessor is an hour, this is a minute. Otherwise walk right the
/// same way; if the first successor is a second or AM/PM marker, this
/// is also a minute. Otherwise it's a month.
fn m_is_minute(raw: &[RawToken], idx: usize) -> bool {
    // Left walk.
    let mut i = idx;
    while i > 0 {
        i -= 1;
        match &raw[i] {
            RawToken::Literal(_) | RawToken::Pad | RawToken::Fill => continue,
            RawToken::H(_) | RawToken::ElapsedH(_) => return true,
            _ => break,
        }
    }
    // Right walk.
    for tok in raw.iter().skip(idx + 1) {
        match tok {
            RawToken::Literal(_) | RawToken::Pad | RawToken::Fill => continue,
            RawToken::S(_) | RawToken::ElapsedS(_) | RawToken::AmPm(_) => return true,
            RawToken::FracSec(_) => return true,
            _ => break,
        }
    }
    false
}

fn find_frac_digits(tokens: &[Token]) -> Option<usize> {
    tokens.iter().find_map(|t| match t {
        Token::FracSecond(n) => Some(*n),
        _ => None,
    })
}

// ---------------------------------------------------------------------------
// DateContext — calendrical decomposition
// ---------------------------------------------------------------------------

struct DateContext {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    frac_second: f64,
    /// 0 = Sunday, 1 = Monday, ..., 6 = Saturday.
    weekday: u32,
}

impl DateContext {
    fn from_serial(
        serial: f64,
        date_1904: bool,
        frac_digits: Option<usize>,
    ) -> Result<Self, FormatWarning> {
        if !serial.is_finite() {
            return Err(FormatWarning::new(
                FormatWarningKind::MalformedFormatCode,
                "",
                serial.to_string(),
                "non-finite date serial",
            ));
        }

        // Round to the precision of the first FracSecond token (or to
        // the whole second otherwise). This keeps rollover consistent
        // at minute / hour / day boundaries: a value that rounds up to
        // 60.000 seconds increments the minute rather than emitting
        // "60.000" in-place.
        let total_seconds_f = serial * 86400.0;
        let scale = match frac_digits {
            None => 1.0,
            Some(d) => 10f64.powi(d as i32),
        };
        let rounded = (total_seconds_f * scale).round() / scale;

        let int_seconds = rounded.floor() as i64;
        let frac_second = (rounded - int_seconds as f64).max(0.0);

        let days = int_seconds.div_euclid(86400);
        let sod = int_seconds.rem_euclid(86400);
        let hour = (sod / 3600) as u32;
        let minute = ((sod % 3600) / 60) as u32;
        let second = (sod % 60) as u32;

        let (year, month, day, weekday) = if date_1904 {
            let jdn = 2_416_481 + days;
            let (y, m, d) = jdn_to_ymd(jdn);
            (y, m, d, weekday_from_jdn(jdn))
        } else if days == 60 {
            // Lotus leap-year bug: Excel 1900 displays serial 60 as
            // 1900-02-29. The linear JDN of 2415020 + 60 = 2415080
            // points at the real-world Mar 1 1900 (Thursday) — we use
            // that weekday so rendering stays monotonic across the
            // bug boundary even though the "day" we print is fake.
            (1900, 2, 29, weekday_from_jdn(2_415_080))
        } else if days < 60 {
            let jdn = 2_415_020 + days;
            let (y, m, d) = jdn_to_ymd(jdn);
            (y, m, d, weekday_from_jdn(jdn))
        } else {
            // Correct the linear offset by one to undo the fake Feb 29.
            let jdn = 2_415_019 + days;
            let (y, m, d) = jdn_to_ymd(jdn);
            (y, m, d, weekday_from_jdn(jdn))
        };

        Ok(DateContext {
            year,
            month,
            day,
            hour,
            minute,
            second,
            frac_second,
            weekday,
        })
    }
}

// ---------------------------------------------------------------------------
// ElapsedContext — duration decomposition
// ---------------------------------------------------------------------------

struct ElapsedContext {
    total_hours: i64,
    total_minutes: i64,
    total_seconds: i64,
    minute_of_hour: i64,
    second_of_minute: i64,
    frac_second: f64,
    /// Echoed for AM/PM tokens that a user might (rarely) wedge into
    /// an elapsed format — behaves as hour-of-day on the last day.
    hour_of_day: u32,
}

impl ElapsedContext {
    fn from_serial(serial: f64, frac_digits: Option<usize>) -> Result<Self, FormatWarning> {
        if !serial.is_finite() {
            return Err(FormatWarning::new(
                FormatWarningKind::MalformedFormatCode,
                "",
                serial.to_string(),
                "non-finite elapsed serial",
            ));
        }

        let total_seconds_f = serial * 86400.0;
        let scale = match frac_digits {
            None => 1.0,
            Some(d) => 10f64.powi(d as i32),
        };
        let rounded = (total_seconds_f * scale).round() / scale;

        let int_seconds = rounded.floor() as i64;
        let frac_second = (rounded - int_seconds as f64).max(0.0);

        let total_seconds = int_seconds;
        let total_minutes = total_seconds.div_euclid(60);
        let total_hours = total_seconds.div_euclid(3600);
        let minute_of_hour = (total_seconds.div_euclid(60)).rem_euclid(60);
        let second_of_minute = total_seconds.rem_euclid(60);
        let hour_of_day = (total_seconds.div_euclid(3600).rem_euclid(24)) as u32;

        Ok(ElapsedContext {
            total_hours,
            total_minutes,
            total_seconds,
            minute_of_hour,
            second_of_minute,
            frac_second,
            hour_of_day,
        })
    }
}

// ---------------------------------------------------------------------------
// Calendar helpers
// ---------------------------------------------------------------------------

/// Fliegel / Van Flandern Gregorian JDN → (year, month, day).
fn jdn_to_ymd(jdn: i64) -> (i64, u32, u32) {
    let l0 = jdn + 68_569;
    let n = (4 * l0) / 146_097;
    let l1 = l0 - (146_097 * n + 3) / 4;
    let i = (4_000 * (l1 + 1)) / 1_461_001;
    let l2 = l1 - (1_461 * i) / 4 + 31;
    let j = (80 * l2) / 2_447;
    let day = (l2 - (2_447 * j) / 80) as u32;
    let l3 = j / 11;
    let month = (j + 2 - 12 * l3) as u32;
    let year = 100 * (n - 49) + i + l3;
    (year, month, day)
}

/// JDN → weekday with 0 = Sunday.
///
/// JDN 2415021 = 1900-01-01, which was a Monday; `(2415021 + 1) % 7 = 1`
/// maps to Monday under the 0-Sunday convention.
fn weekday_from_jdn(jdn: i64) -> u32 {
    (jdn + 1).rem_euclid(7) as u32
}

const MONTH_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const MONTH_LONG: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const MONTH_FIRST: [char; 12] = ['J', 'F', 'M', 'A', 'M', 'J', 'J', 'A', 'S', 'O', 'N', 'D'];
const WEEKDAY_SHORT: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const WEEKDAY_LONG: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

// ---------------------------------------------------------------------------
// Emit
// ---------------------------------------------------------------------------

fn emit_date_token(tok: &Token, ctx: &DateContext, has_am_pm: bool, out: &mut String) {
    match tok {
        Token::Year(n) => {
            if *n <= 2 {
                out.push_str(&format!("{:02}", ctx.year.rem_euclid(100)));
            } else {
                out.push_str(&format!("{:04}", ctx.year));
            }
        }
        Token::MonthNumeric(n) => {
            if *n == 1 {
                out.push_str(&ctx.month.to_string());
            } else {
                out.push_str(&format!("{:02}", ctx.month));
            }
        }
        Token::MonthShortName => {
            out.push_str(MONTH_SHORT[(ctx.month - 1) as usize]);
        }
        Token::MonthLongName => {
            out.push_str(MONTH_LONG[(ctx.month - 1) as usize]);
        }
        Token::MonthFirstLetter => {
            out.push(MONTH_FIRST[(ctx.month - 1) as usize]);
        }
        Token::Day(n) => {
            if *n == 1 {
                out.push_str(&ctx.day.to_string());
            } else {
                out.push_str(&format!("{:02}", ctx.day));
            }
        }
        Token::WeekdayShort => out.push_str(WEEKDAY_SHORT[ctx.weekday as usize]),
        Token::WeekdayLong => out.push_str(WEEKDAY_LONG[ctx.weekday as usize]),
        Token::Hour(n) => {
            let h = display_hour(ctx.hour, has_am_pm);
            if *n == 1 {
                out.push_str(&h.to_string());
            } else {
                out.push_str(&format!("{:02}", h));
            }
        }
        Token::Minute(n) => {
            if *n == 1 {
                out.push_str(&ctx.minute.to_string());
            } else {
                out.push_str(&format!("{:02}", ctx.minute));
            }
        }
        Token::Second(n) => {
            if *n == 1 {
                out.push_str(&ctx.second.to_string());
            } else {
                out.push_str(&format!("{:02}", ctx.second));
            }
        }
        Token::FracSecond(n) => emit_frac_second(ctx.frac_second, *n, out),
        Token::AmPm(style) => out.push_str(am_pm_text(style, ctx.hour < 12)),
        // Elapsed tokens accidentally in date context — render
        // defensively as if they were their non-bracketed counterpart.
        Token::ElapsedHour(n) => {
            let h = display_hour(ctx.hour, has_am_pm);
            out.push_str(&format!("{:0width$}", h, width = *n));
        }
        Token::ElapsedMinute(n) => {
            out.push_str(&format!("{:0width$}", ctx.minute, width = *n));
        }
        Token::ElapsedSecond(n) => {
            out.push_str(&format!("{:0width$}", ctx.second, width = *n));
        }
        Token::Literal(s) => out.push_str(s),
        Token::Pad => out.push(' '),
        Token::Fill => {}
    }
}

fn emit_elapsed_token(tok: &Token, ctx: &ElapsedContext, out: &mut String) {
    match tok {
        Token::ElapsedHour(n) => emit_elapsed_number(ctx.total_hours, *n, out),
        Token::ElapsedMinute(n) => emit_elapsed_number(ctx.total_minutes, *n, out),
        Token::ElapsedSecond(n) => emit_elapsed_number(ctx.total_seconds, *n, out),
        Token::Hour(n) => {
            // Modular hour-of-day for plain `h` inside an elapsed body.
            if *n == 1 {
                out.push_str(&ctx.hour_of_day.to_string());
            } else {
                out.push_str(&format!("{:02}", ctx.hour_of_day));
            }
        }
        Token::Minute(n) => {
            if *n == 1 {
                out.push_str(&ctx.minute_of_hour.to_string());
            } else {
                out.push_str(&format!("{:02}", ctx.minute_of_hour));
            }
        }
        Token::Second(n) => {
            if *n == 1 {
                out.push_str(&ctx.second_of_minute.to_string());
            } else {
                out.push_str(&format!("{:02}", ctx.second_of_minute));
            }
        }
        Token::FracSecond(n) => emit_frac_second(ctx.frac_second, *n, out),
        Token::AmPm(style) => out.push_str(am_pm_text(style, ctx.hour_of_day < 12)),
        Token::Literal(s) => out.push_str(s),
        Token::Pad => out.push(' '),
        Token::Fill => {}
        // Calendrical tokens in elapsed context — emit nothing rather
        // than raise, since elapsed bodies with y/m/d are typically
        // junk formats not worth a hard failure.
        Token::Year(_)
        | Token::MonthNumeric(_)
        | Token::MonthShortName
        | Token::MonthLongName
        | Token::MonthFirstLetter
        | Token::Day(_)
        | Token::WeekdayShort
        | Token::WeekdayLong => {}
    }
}

/// Excel 12-hour wheel: 0 → 12, 13..23 → 1..11, 12 → 12, 1..11 unchanged.
fn display_hour(hour: u32, has_am_pm: bool) -> u32 {
    if !has_am_pm {
        return hour;
    }
    match hour {
        0 => 12,
        1..=12 => hour,
        _ => hour - 12,
    }
}

fn am_pm_text(style: &AmPmStyle, is_am: bool) -> &'static str {
    match style {
        AmPmStyle::UpperFull => {
            if is_am {
                "AM"
            } else {
                "PM"
            }
        }
        AmPmStyle::LowerFull => {
            if is_am {
                "am"
            } else {
                "pm"
            }
        }
        AmPmStyle::UpperLetter => {
            if is_am {
                "A"
            } else {
                "P"
            }
        }
        AmPmStyle::LowerLetter => {
            if is_am {
                "a"
            } else {
                "p"
            }
        }
    }
}

fn emit_frac_second(frac_second: f64, digits: usize, out: &mut String) {
    out.push('.');
    let scale = 10f64.powi(digits as i32);
    // The context rounding already matches this precision, so a
    // simple round-to-int is sufficient and keeps us from depending
    // on format!'s half-to-even rule.
    let scaled = (frac_second * scale).round() as i64;
    out.push_str(&format!("{:0width$}", scaled, width = digits));
}

fn emit_elapsed_number(value: i64, min_width: usize, out: &mut String) {
    // Elapsed values can legitimately overflow `min_width` (e.g. [h]
    // with 37 hours); format! wider than requested if needed, otherwise
    // zero-pad to min_width.
    let s = value.to_string();
    if s.len() < min_width {
        for _ in 0..(min_width - s.len()) {
            out.push('0');
        }
    }
    out.push_str(&s);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(body: &str, serial: f64) -> String {
        format_date_body(serial, body, false).expect("date body should format")
    }

    fn fmt1904(body: &str, serial: f64) -> String {
        format_date_body(serial, body, true).expect("date body should format")
    }

    fn fmt_elapsed(body: &str, serial: f64) -> String {
        format_elapsed_body(serial, body).expect("elapsed body should format")
    }

    // --- Epoch alignment ----------------------------------------------------

    #[test]
    fn serial_one_is_nineteen_hundred_jan_first() {
        assert_eq!(fmt("yyyy-mm-dd", 1.0), "1900-01-01");
    }

    #[test]
    fn serial_fifty_nine_is_feb_twenty_eight_nineteen_hundred() {
        assert_eq!(fmt("yyyy-mm-dd", 59.0), "1900-02-28");
    }

    #[test]
    fn lotus_leap_year_bug_displays_feb_twenty_ninth() {
        // Serial 60 in the 1900 system is Excel's fake Feb 29, 1900.
        assert_eq!(fmt("yyyy-mm-dd", 60.0), "1900-02-29");
    }

    #[test]
    fn serial_sixty_one_is_march_first_after_bug() {
        assert_eq!(fmt("yyyy-mm-dd", 61.0), "1900-03-01");
    }

    #[test]
    fn nineteen_oh_four_epoch_zero_is_jan_first() {
        assert_eq!(fmt1904("yyyy-mm-dd", 0.0), "1904-01-01");
    }

    #[test]
    fn twenty_twenty_six_serial_renders_correctly() {
        // Serial 46115 = 2026-04-03 in the 1900 system. Confirms
        // alignment with the legacy path used by the mod-level scaffold.
        assert_eq!(fmt("yyyy-mm-dd", 46115.0), "2026-04-03");
    }

    // --- Year / month / day -------------------------------------------------

    #[test]
    fn two_digit_year_is_last_two_digits() {
        assert_eq!(fmt("yy", 46115.0), "26");
    }

    #[test]
    fn month_numeric_respects_pad() {
        assert_eq!(fmt("m", 46115.0), "4"); // April
        assert_eq!(fmt("mm", 46115.0), "04");
    }

    #[test]
    fn month_short_name() {
        assert_eq!(fmt("mmm", 46115.0), "Apr");
    }

    #[test]
    fn month_long_name() {
        assert_eq!(fmt("mmmm", 46115.0), "April");
    }

    #[test]
    fn month_first_letter() {
        assert_eq!(fmt("mmmmm", 46115.0), "A");
        // March = M
        let march_serial = 46082.0; // 2026-03-01
        assert_eq!(fmt("mmmmm", march_serial), "M");
    }

    #[test]
    fn day_numeric_respects_pad() {
        assert_eq!(fmt("d", 46115.0), "3");
        assert_eq!(fmt("dd", 46115.0), "03");
    }

    #[test]
    fn weekday_short_name() {
        // 2026-04-03 is a Friday.
        assert_eq!(fmt("ddd", 46115.0), "Fri");
    }

    #[test]
    fn weekday_long_name() {
        assert_eq!(fmt("dddd", 46115.0), "Friday");
    }

    // --- Hour / minute / second + AM/PM ------------------------------------

    #[test]
    fn hour_minute_twenty_four_hour() {
        // 0.5 = noon exactly.
        assert_eq!(fmt("hh:mm", 0.5), "12:00");
    }

    #[test]
    fn hour_minute_past_midnight() {
        // 0.25 = 6:00 AM.
        assert_eq!(fmt("h:mm", 0.25), "6:00");
    }

    #[test]
    fn watchlist_canary_h_mm_am_pm() {
        // THE target defect: `h:mm AM/PM` on 0.3958333 must render
        // "9:30 AM". Previously emitted "1899-12-31T09:30:00".
        let serial = 9.0 / 24.0 + 30.0 / 1440.0; // 9:30 AM
        assert_eq!(fmt("h:mm AM/PM", serial), "9:30 AM");
    }

    #[test]
    fn hour_zero_renders_as_twelve_in_am_pm() {
        // Midnight in 12-hour wheel.
        assert_eq!(fmt("h:mm AM/PM", 0.0), "12:00 AM");
    }

    #[test]
    fn hour_thirteen_renders_as_one_pm() {
        let serial = 13.0 / 24.0; // 1:00 PM
        assert_eq!(fmt("h:mm AM/PM", serial), "1:00 PM");
    }

    #[test]
    fn lowercase_am_pm_is_lowercase() {
        let serial = 9.0 / 24.0 + 30.0 / 1440.0;
        assert_eq!(fmt("h:mm am/pm", serial), "9:30 am");
    }

    #[test]
    fn single_letter_am_pm() {
        let serial = 9.0 / 24.0;
        assert_eq!(fmt("h A/P", serial), "9 A");
        let pm_serial = 15.0 / 24.0;
        assert_eq!(fmt("h A/P", pm_serial), "3 P");
        assert_eq!(fmt("h a/p", pm_serial), "3 p");
    }

    // --- Second + fractional ------------------------------------------------

    #[test]
    fn second_pad() {
        let serial = 12.0 / 86400.0; // 12 seconds past midnight
        assert_eq!(fmt("s", serial), "12");
        let single = 5.0 / 86400.0;
        assert_eq!(fmt("s", single), "5");
        assert_eq!(fmt("ss", single), "05");
    }

    #[test]
    fn fractional_seconds_three_digits() {
        // 12.345 seconds since midnight.
        let serial = 12.345 / 86400.0;
        assert_eq!(fmt("ss.000", serial), "12.345");
    }

    #[test]
    fn fractional_seconds_rounding_rollover_to_next_minute() {
        // 59.9996 seconds rounds to 60.000 at 3 digits → rolls over
        // to the next minute. With format "mm:ss.000", we expect
        // "01:00.000".
        let serial = 59.9996 / 86400.0;
        assert_eq!(fmt("mm:ss.000", serial), "01:00.000");
    }

    // --- Full combos --------------------------------------------------------

    #[test]
    fn full_timestamp() {
        // 2026-04-03 14:30:15 → serial 46115 + 14/24 + 30/1440 + 15/86400
        let serial = 46115.0 + 14.0 / 24.0 + 30.0 / 1440.0 + 15.0 / 86400.0;
        assert_eq!(fmt("yyyy-mm-dd hh:mm:ss", serial), "2026-04-03 14:30:15");
    }

    #[test]
    fn verbose_weekday_month_day_year() {
        // 2026-04-03 is Friday.
        assert_eq!(fmt("dddd, mmmm d, yyyy", 46115.0), "Friday, April 3, 2026");
    }

    // --- Ambiguous m resolution --------------------------------------------

    #[test]
    fn mm_after_h_is_minute() {
        // "h:mm" at 9:05 AM: h (1-digit) → "9", mm (2-digit) → "05".
        // The positional rule resolves mm as minute because the
        // predecessor `h` is an hour token.
        let serial = 9.0 / 24.0 + 5.0 / 1440.0;
        assert_eq!(fmt("h:mm", serial), "9:05");
    }

    #[test]
    fn mm_between_hh_and_ss_is_minute() {
        let serial = 9.0 / 24.0 + 30.0 / 1440.0 + 45.0 / 86400.0;
        assert_eq!(fmt("hh:mm:ss", serial), "09:30:45");
    }

    #[test]
    fn mm_after_y_dash_is_month() {
        // "yyyy-mm-dd" — mm has no hour/second neighbour so it must
        // be month.
        assert_eq!(fmt("yyyy-mm-dd", 46115.0), "2026-04-03");
    }

    #[test]
    fn mm_before_am_pm_is_minute() {
        // "mm AM/PM" with serial 0.25 (6:00 AM) — successor is AM/PM,
        // so mm is minute → "00 AM".
        assert_eq!(fmt("mm AM/PM", 0.25), "00 AM");
    }

    // --- Literals & escapes ------------------------------------------------

    #[test]
    fn quoted_literal_renders_verbatim() {
        assert_eq!(fmt("\"Date: \"yyyy", 46115.0), "Date: 2026");
    }

    #[test]
    fn backslash_escape_renders_next_char() {
        // \D yyyy — the D is a literal, not a day token.
        assert_eq!(fmt(r"\Dyyyy", 46115.0), "D2026");
    }

    #[test]
    fn underscore_pad_emits_space() {
        assert_eq!(fmt("yyyy_m", 46115.0), "2026 ");
        // _m: `_` consumes the following char as a width hint, emits
        // a space. The token following in the body is whatever comes
        // next — here, nothing, so "2026 " is correct.
    }

    #[test]
    fn star_fill_is_dropped() {
        assert_eq!(fmt("yyyy*_", 46115.0), "2026");
    }

    // --- Elapsed -----------------------------------------------------------

    #[test]
    fn elapsed_hours_display_total() {
        // 1.5 days = 36 hours exactly.
        assert_eq!(fmt_elapsed("[h]:mm:ss", 1.5), "36:00:00");
    }

    #[test]
    fn elapsed_hours_zero_padded_to_two_digits_minimum() {
        // 0.5 days = 12 hours; [hh] requests minimum 2-digit pad.
        assert_eq!(fmt_elapsed("[hh]:mm:ss", 0.5), "12:00:00");
    }

    #[test]
    fn elapsed_minutes_display_total() {
        // 1.5 hours = 90 minutes.
        let serial = 1.5 / 24.0;
        assert_eq!(fmt_elapsed("[m]:ss", serial), "90:00");
    }

    #[test]
    fn elapsed_seconds_display_total() {
        // 1 hour = 3600 seconds.
        let serial = 1.0 / 24.0;
        assert_eq!(fmt_elapsed("[s]", serial), "3600");
    }

    #[test]
    fn elapsed_with_fractional_seconds() {
        // 1.5 hours + 0.250s = 5400.25 seconds total.
        let serial = (5400.25) / 86400.0;
        assert_eq!(fmt_elapsed("[s].000", serial), "5400.250");
    }

    #[test]
    fn elapsed_hours_with_minute_and_second_modular() {
        // 1 day + 2h + 3m + 4s = 26h 3m 4s total.
        let serial = 1.0 + 2.0 / 24.0 + 3.0 / 1440.0 + 4.0 / 86400.0;
        assert_eq!(fmt_elapsed("[h]:mm:ss", serial), "26:03:04");
    }

    // --- Malformed ---------------------------------------------------------

    #[test]
    fn unterminated_quote_returns_malformed() {
        let err = format_date_body(46115.0, "yyyy-\"mm", false).unwrap_err();
        assert_eq!(err.kind, FormatWarningKind::MalformedFormatCode);
    }

    #[test]
    fn unterminated_bracket_returns_malformed() {
        let err = format_date_body(46115.0, "[h:mm", false).unwrap_err();
        assert_eq!(err.kind, FormatWarningKind::MalformedFormatCode);
    }

    #[test]
    fn trailing_backslash_returns_malformed() {
        let err = format_date_body(46115.0, "yyyy\\", false).unwrap_err();
        assert_eq!(err.kind, FormatWarningKind::MalformedFormatCode);
    }

    #[test]
    fn non_finite_serial_returns_malformed() {
        let err = format_date_body(f64::NAN, "yyyy-mm-dd", false).unwrap_err();
        assert_eq!(err.kind, FormatWarningKind::MalformedFormatCode);
    }
}
