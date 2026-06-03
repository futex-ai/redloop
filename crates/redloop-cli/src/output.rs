pub(crate) fn print_json<T: serde::Serialize>(value: &T) {
    let serialized = serde_json::to_string_pretty(value).unwrap_or_else(|_| "null".to_string());
    println!("{serialized}");
}
