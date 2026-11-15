use std::collections::HashMap;

pub struct Lang {
    tr: HashMap<String, String>,
    en: HashMap<String, String>,
    current: String,
}

impl Lang {
    pub fn new(language: &str) -> Self {
        let tr: HashMap<String, String> =
            serde_json::from_str(include_str!("../assets/lang_tr.json")).unwrap_or_default();
        let en: HashMap<String, String> =
            serde_json::from_str(include_str!("../assets/lang_en.json")).unwrap_or_default();
        Self {
            tr,
            en,
            current: if language.eq_ignore_ascii_case("en") {
                "en".into()
            } else {
                "tr".into()
            },
        }
    }

    pub fn language(&self) -> &str {
        &self.current
    }

    pub fn set_language(&mut self, language: &str) {
        self.current = if language.eq_ignore_ascii_case("en") {
            "en".into()
        } else {
            "tr".into()
        };
    }

    pub fn t(&self, key: &str) -> String {
        let map = if self.current == "en" { &self.en } else { &self.tr };
        map.get(key)
            .cloned()
            .or_else(|| self.tr.get(key).cloned())
            .unwrap_or_else(|| key.to_string())
    }

    pub fn tf(&self, key: &str, args: &[&str]) -> String {
        let mut text = self.t(key);
        for (i, arg) in args.iter().enumerate() {
            text = text.replace(&format!("{{{i}}}"), arg);
        }
        text
    }

}
