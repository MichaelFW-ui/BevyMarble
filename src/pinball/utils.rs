/// 格式化数值，显示单位 K/M/B
pub fn format_value(value: u64) -> String {
    const MAX_VALUE: u64 = 32_000_000_000;
    let value = value.min(MAX_VALUE);

    if value >= 1_000_000_000 {
        format!("{}B", value / 1_000_000_000)
    } else if value >= 1_000_000 {
        format!("{}M", value / 1_000_000)
    } else if value >= 1_000 {
        format!("{}K", value / 1_000)
    } else {
        format!("{}", value)
    }
}

/// 根据数值计算小球半径
pub fn calculate_radius(value: u64) -> f32 {
    const BASE_RADIUS: f32 = 6.0;
    const MAX_RADIUS: f32 = 15.0;  // 限制最大半径，避免卡住
    const MAX_VALUE: u64 = 32_000_000_000;

    let value = value.min(MAX_VALUE);
    if value <= 2 {
        return BASE_RADIUS;
    }
    let ratio = (value as f32).log10() / (MAX_VALUE as f32).log10();
    BASE_RADIUS + (MAX_RADIUS - BASE_RADIUS) * ratio
}

/// 最大数值限制（32B）
pub const MAX_VALUE: u64 = 32_000_000_000;
