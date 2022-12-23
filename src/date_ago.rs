use nostr_types::Unixtime;

pub fn date_ago(then: Unixtime) -> String {
    let now = Unixtime::now().unwrap();
    let seconds = now.0 - then.0;
    let minutes: f32 = seconds as f32 / 60.0;
    let hours: f32 = minutes / 60.0;
    let days: f32 = hours / 24.0;
    let years: f32 = days / 365.0;

    if seconds < 45 {
        format!("{}s", seconds)
    } else if seconds < 90 {
        "1m".to_string()
    } else if minutes < 45.0 {
        format!("{}m", minutes as i64)
    } else if minutes < 90.0 {
        "1h".to_string()
    } else if hours < 24.0 {
        format!("{}h", hours as i64)
    } else if hours < 42.0 {
        "1d".to_string()
    } else if days < 30.0 {
        format!("{}d", days as i64)
    } else if days < 45.0 {
        "1m".to_string()
    } else if days < 365.0 {
        format!("{}m", (days / 30.0) as i64)
    } else if years < 1.5 {
        "1y".to_string()
    } else {
        format!("{}y", years as i64)
    }
}
