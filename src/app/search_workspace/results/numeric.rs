#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is explicitly floored and bounded to the u32 range"
)]
pub(super) fn floor_to_u32(value: f64) -> u32 {
    if value.is_nan() || value <= 0.0 {
        return 0;
    }
    value.floor().min(f64::from(u32::MAX)) as u32
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is explicitly rounded up and bounded to the u32 range"
)]
pub(super) fn ceil_to_u32(value: f64) -> u32 {
    if value.is_nan() || value <= 0.0 {
        return 0;
    }
    value.ceil().min(f64::from(u32::MAX)) as u32
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "CSSOM exposes fractional coordinates while DOM positioning accepts integer pixels; the value is explicitly bounded"
)]
pub(super) fn trunc_to_i32(value: f64) -> i32 {
    if value.is_nan() {
        return 0;
    }
    value
        .trunc()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::{ceil_to_u32, floor_to_u32, trunc_to_i32};

    #[test]
    fn unsigned_conversions_define_rounding_and_bounds() {
        assert_eq!(floor_to_u32(12.9), 12);
        assert_eq!(ceil_to_u32(12.1), 13);
        assert_eq!(floor_to_u32(-1.0), 0);
        assert_eq!(ceil_to_u32(f64::NAN), 0);
        assert_eq!(ceil_to_u32(f64::INFINITY), u32::MAX);
    }

    #[test]
    fn signed_conversion_truncates_and_saturates() {
        assert_eq!(trunc_to_i32(12.9), 12);
        assert_eq!(trunc_to_i32(-12.9), -12);
        assert_eq!(trunc_to_i32(f64::NAN), 0);
        assert_eq!(trunc_to_i32(f64::INFINITY), i32::MAX);
        assert_eq!(trunc_to_i32(f64::NEG_INFINITY), i32::MIN);
    }
}
