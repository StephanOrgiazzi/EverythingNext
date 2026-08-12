const FILE_SIZE_UNITS: [&str; 5] = ["o", "Ko", "Mo", "Go", "To"];

pub(crate) fn file_size(size: Option<u64>, is_dir: bool) -> String {
    if is_dir {
        return String::new();
    }
    let Some(bytes) = size else {
        return "—".into();
    };

    let mut unit = 0;
    let mut divisor = 1_u64;
    while bytes / divisor >= 1024 && unit < FILE_SIZE_UNITS.len() - 1 {
        divisor *= 1024;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, FILE_SIZE_UNITS[unit])
    } else {
        let whole = bytes / divisor;
        let remainder = bytes % divisor;
        let rounded_tenths = (remainder * 10 + divisor / 2) / divisor;
        let whole = whole + rounded_tenths / 10;
        let decimal = rounded_tenths % 10;
        format!("{whole}.{decimal} {}", FILE_SIZE_UNITS[unit])
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "JavaScript Date accepts milliseconds as f64; Unix timestamps in the supported UI range remain precise enough for minute display"
)]
pub(crate) fn modified_date(timestamp: Option<i64>) -> String {
    let Some(timestamp) = timestamp else {
        return "—".into();
    };
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(timestamp as f64 * 1000.0));
    format!(
        "{:02}/{:02}/{} {:02}:{:02}",
        date.get_date(),
        date.get_month() + 1,
        date.get_full_year(),
        date.get_hours(),
        date.get_minutes(),
    )
}

pub(crate) fn result_count(total: u32) -> String {
    let digits = total.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(' ');
        }
        grouped.push(digit);
    }
    let label = if total == 1 { "result" } else { "results" };
    format!("{} {label}", grouped.chars().rev().collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::{file_size, result_count};

    #[test]
    fn formats_file_sizes_without_changing_existing_units() {
        assert_eq!(file_size(None, false), "—");
        assert_eq!(file_size(Some(512), false), "512 o");
        assert_eq!(file_size(Some(1536), false), "1.5 Ko");
        assert_eq!(file_size(Some(1536), true), "");
    }

    #[test]
    fn groups_result_counts_by_thousands() {
        assert_eq!(result_count(0), "0 results");
        assert_eq!(result_count(12_480), "12 480 results");
    }

    #[test]
    fn uses_singular_for_one_result() {
        assert_eq!(result_count(1), "1 result");
    }
}
