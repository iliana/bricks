pub(crate) fn emoji(s: &str) -> askama::Result<String> {
    Ok(s.strip_prefix("0x")
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .and_then(|s| char::try_from(s).ok())
        .map(String::from)
        .unwrap_or_else(|| s.into()))
}
