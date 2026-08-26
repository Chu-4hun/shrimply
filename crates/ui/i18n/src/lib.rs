use rust_i18n::t;
use std::borrow::Cow;

rust_i18n::i18n!("locales", fallback = "en");

const DEFAULT_LOCALE: &str = "en";
const SUPPORTED_LOCALES: [&str; 5] = ["en", "es", "fr", "de", "ja"];

pub fn init_system_locale() {
    let locale = glib::language_names()
        .iter()
        .map(|locale| normalize_locale(locale))
        .find(|locale| SUPPORTED_LOCALES.contains(&locale.as_str()))
        .unwrap_or_else(|| DEFAULT_LOCALE.to_string());
    rust_i18n::set_locale(&locale);
}

pub fn text(key: &str) -> Cow<'_, str> {
    t!(key)
}

pub fn text_args(key: &str, args: &[(&str, String)]) -> String {
    let translated = text(key);
    let patterns = args.iter().map(|(name, _)| *name).collect::<Vec<_>>();
    let values = args
        .iter()
        .map(|(_, value)| value.clone())
        .collect::<Vec<_>>();
    rust_i18n::replace_patterns(&translated, &patterns, &values)
}

fn normalize_locale(locale: &str) -> String {
    locale
        .split(['.', '@'])
        .next()
        .unwrap_or(locale)
        .split(['_', '-'])
        .next()
        .unwrap_or(locale)
        .to_ascii_lowercase()
}
