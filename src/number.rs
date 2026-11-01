//! Floats as decimals (SPEC §6.7).

/// The fewest significant digits that read back as a positive, finite float, ties to even:
/// `value = digits × 10^exponent`.
pub(crate) fn shortest(value: f64) -> (String, i32) {
    let short = format!("{value:e}");
    let precision = short
        .split('e')
        .next()
        .unwrap_or_default()
        .len()
        .saturating_sub(2);
    let even = format!("{value:.precision$e}");
    let text = if even.parse() == Ok(value) {
        even
    } else {
        short
    };
    let (mantissa, exponent) = text.split_once('e').expect("LowerExp has an exponent");
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let exponent: i32 = exponent.parse().expect("LowerExp exponent is an integer");
    (
        format!("{whole}{fraction}"),
        exponent - fraction.len() as i32,
    )
}

/// The shortest decimal, without exponent, that reads back as the same float.
pub(crate) fn format(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    let sign = if value < 0.0 { "-" } else { "" };
    let (digits, exponent) = shortest(value.abs());
    let point = digits.len() as i32 + exponent;
    if exponent >= 0 {
        format!("{sign}{digits}{}", "0".repeat(exponent as usize))
    } else if point > 0 {
        let (whole, fraction) = digits.split_at(point as usize);
        format!("{sign}{whole}.{fraction}")
    } else {
        format!("{sign}0.{}{digits}", "0".repeat(-point as usize))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(format(1e22), "10000000000000000000000");
        assert_eq!(format(1.5e-7), "0.00000015");
        assert_eq!(format(-0.0), "0");
        assert_eq!(format(0.1), "0.1");
        assert_eq!(format(2f64.powi(-24)), "0.00000005960464477539063");
        assert_eq!(format(123.456), "123.456");
        assert_eq!(format(-2.5), "-2.5");
        assert_eq!(format(100.0), "100");
        assert_eq!(format(9007199254740992.0), "9007199254740992");
        assert_eq!(format(5e-324), format!("0.{}5", "0".repeat(323)));
        assert_eq!(
            format(f64::MAX),
            format!("17976931348623157{}", "0".repeat(292))
        );
    }

    #[test]
    fn ties_go_to_even() {
        let tie: f64 = "1801514316094494.25".parse().unwrap();
        assert_eq!(format(tie), "1801514316094494.2");
        assert_eq!(shortest(tie), ("18015143160944942".to_string(), -1));
    }
}
