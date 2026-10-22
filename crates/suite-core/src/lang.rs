//! The UI languages and the `lang` setting. `lang = "auto"` (the default) follows the system; an explicit tag wins.

/// A UI language. `UzLatn` is the alphabet in force (oʻ gʻ sh ch); `UzLatn2026` is the 2026 reform (ö ğ ş ç).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    En,
    Ru,
    UzLatn,
    UzLatn2026,
    UzCyrl,
}

impl Lang {
    pub const ALL: [Lang; 5] = [
        Lang::En,
        Lang::Ru,
        Lang::UzLatn,
        Lang::UzLatn2026,
        Lang::UzCyrl,
    ];

    /// The tag written to `settings.toml` and used by the catalogs.
    pub const fn tag(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ru => "ru",
            Lang::UzLatn => "uz-Latn",
            Lang::UzLatn2026 => "uz-Latn-2026",
            Lang::UzCyrl => "uz-Cyrl",
        }
    }

    /// Exactly one of the five tags (what `settings.toml` may hold).
    pub fn from_tag(s: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|l| l.tag() == s)
    }

    /// A system locale name (`uz-Latn-UZ`, `ru_RU`, `en-US`, `uz`) mapped to the nearest language; `None` for any other language.
    /// An Uzbek locale without a script is Latin.
    pub fn from_locale(s: &str) -> Option<Lang> {
        let s = s
            .split(['.', '@'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .replace('_', "-");
        let mut parts = s.split('-');
        match parts.next()? {
            "en" => Some(Lang::En),
            "ru" => Some(Lang::Ru),
            "uz" => {
                let rest: Vec<&str> = parts.collect();
                Some(if rest.contains(&"cyrl") {
                    Lang::UzCyrl
                } else if rest.contains(&"2026") {
                    Lang::UzLatn2026
                } else {
                    Lang::UzLatn
                })
            }
            _ => None,
        }
    }
}

/// The `lang` setting: follow the system, or one language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LangChoice {
    #[default]
    Auto,
    Fixed(Lang),
}

impl LangChoice {
    pub fn parse(s: &str) -> Option<LangChoice> {
        if s == "auto" {
            Some(LangChoice::Auto)
        } else {
            Lang::from_tag(s).map(LangChoice::Fixed)
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            LangChoice::Auto => "auto",
            LangChoice::Fixed(l) => l.tag(),
        }
    }

    /// The language to show. `Auto` follows `system` (the caller reads it: the app's own locale detection); a system language
    /// we have no catalog for, or none at all, gives English.
    pub fn resolve(self, system: Option<&str>) -> Lang {
        match self {
            LangChoice::Fixed(l) => l,
            LangChoice::Auto => system.and_then(Lang::from_locale).unwrap_or(Lang::En),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_round_trip_and_only_the_five_parse() {
        for l in Lang::ALL {
            assert_eq!(Lang::from_tag(l.tag()), Some(l));
            assert_eq!(LangChoice::parse(l.tag()), Some(LangChoice::Fixed(l)));
        }
        assert_eq!(LangChoice::parse("auto"), Some(LangChoice::Auto));
        for bad in ["", "EN", "uz", "uz-latn", "de", "auto ", "uz-Latn-2025"] {
            assert_eq!(LangChoice::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn system_locales_map_to_the_nearest_language() {
        for (loc, want) in [
            ("en-US", Lang::En),
            ("en_GB.UTF-8", Lang::En),
            ("ru-RU", Lang::Ru),
            ("ru_RU@euro", Lang::Ru),
            ("uz", Lang::UzLatn),
            ("uz-UZ", Lang::UzLatn),
            ("uz-Latn-UZ", Lang::UzLatn),
            ("uz-Cyrl-UZ", Lang::UzCyrl),
            ("uz_Cyrl", Lang::UzCyrl),
            ("UZ-LATN-2026", Lang::UzLatn2026),
        ] {
            assert_eq!(Lang::from_locale(loc), Some(want), "{loc}");
        }
        assert_eq!(Lang::from_locale("de-DE"), None);
        assert_eq!(Lang::from_locale(""), None);
    }

    #[test]
    fn auto_follows_the_system_and_a_choice_wins() {
        assert_eq!(LangChoice::Auto.resolve(Some("uz-Cyrl-UZ")), Lang::UzCyrl);
        assert_eq!(LangChoice::Auto.resolve(Some("fr-FR")), Lang::En);
        assert_eq!(LangChoice::Auto.resolve(None), Lang::En);
        assert_eq!(
            LangChoice::Fixed(Lang::Ru).resolve(Some("uz-Latn")),
            Lang::Ru
        );
        assert_eq!(
            LangChoice::Fixed(Lang::UzLatn2026).resolve(None),
            Lang::UzLatn2026
        );
    }
}
