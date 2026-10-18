//! Currency display: a code becomes the symbol a reader recognises, and the
//! Console API's microcent amounts become dollars.

/// Microcents per US dollar: the unit the OpenCode Console API reports money in
/// (100,000,000 per dollar), so integrations do not round fractions of a cent.
pub const MICRO_CENTS_PER_DOLLAR: i128 = 100_000_000;

/// Currency code to symbol, falling back to the code itself.
pub fn symbol(currency: &str) -> String {
    match currency {
        "CNY" | "RMB" => "¥".to_string(),
        "USD" => "$".to_string(),
        "EUR" => "€".to_string(),
        "HKD" => "HK$".to_string(),
        // An unknown code needs a separator, or `JPY1000` reads as one token.
        other => format!("{other} "),
    }
}

/// Parses a decimal microcents string, as the Console API returns them.
///
/// `None` when the value is not a whole number of microcents (the API never
/// sends one, but a sloppy proxy might).
pub fn parse_micro_cents(value: &str) -> Option<i128> {
    value.trim().parse::<i128>().ok()
}

/// Microcents as US dollars with two decimals, e.g. `$62.75`.
///
/// Fractions of a cent are dropped (integer division): spend is displayed, not
/// settled, and the API's own precision is far below what a person reads.
pub fn usd(micro_cents: i128) -> String {
    let sign = if micro_cents < 0 { "-" } else { "" };
    let cents = micro_cents.unsigned_abs() / 1_000_000;
    format!("{sign}${}.{:02}", cents / 100, cents % 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_codes_get_symbols_and_unknown_ones_a_separator() {
        assert_eq!(symbol("CNY"), "¥");
        assert_eq!(symbol("RMB"), "¥");
        assert_eq!(symbol("USD"), "$");
        assert_eq!(symbol("EUR"), "€");
        assert_eq!(symbol("HKD"), "HK$");
        assert_eq!(symbol("JPY"), "JPY ");
    }

    #[test]
    fn microcents_parse_and_render_as_dollars() {
        assert_eq!(parse_micro_cents(" 7525000000 "), Some(7_525_000_000));
        assert_eq!(parse_micro_cents("not a number"), None);

        assert_eq!(usd(7_525_000_000), "$75.25");
        assert_eq!(usd(1_250_000_000), "$12.50");
        assert_eq!(usd(0), "$0.00");
        // Cents are kept, sub-cent fractions are dropped, a deficit is signed.
        assert_eq!(usd(7_525_700_000), "$75.25");
        assert_eq!(usd(-1_250_000_000), "-$12.50");
        assert_eq!(MICRO_CENTS_PER_DOLLAR, 100_000_000);
    }
}
