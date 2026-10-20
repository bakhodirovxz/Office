//! JS bindings of `uz-translit`. This crate only maps types: strings in, strings out, option objects to `Options`.
use uz_translit::{Apostrophe, Direction, LangPolicy, LatinAlphabet, Options, Run, Script};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TS: &'static str = r#"
export type Direction = "cyrl" | "latn";
export interface TranslitOptions {
  /** Latin output apostrophes: U+02BB/U+02BC ("official", default) or U+0027 ("ascii"). */
  apostrophe?: "official" | "ascii";
  /** Which Latin alphabet is written (->latn) or read (->cyrl). Default "1995". */
  latinAlphabet?: "1995" | "2026" | "auto";
  /** ->cyrl: copy words with c (not in ch), w or diacritics unchanged. Default true. */
  foreignGuard?: boolean;
  /** ->cyrl: protect Roman numerals. Default true. */
  romanNumerals?: boolean;
  /** convertRuns only, for runs given as {text, lang}: "convert-ru-en" (default) or "skip-ru-en". */
  langPolicy?: "convert-ru-en" | "skip-ru-en";
}
/** A run of one paragraph; `lang` is a BCP-47 tag that `langPolicy` decides on. A plain string is an untagged run. */
export type Run = string | { text: string; lang?: string | null };
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "TranslitOptions | undefined")]
    pub type Opts;
    #[wasm_bindgen(method, getter)]
    fn apostrophe(this: &Opts) -> Option<String>;
    #[wasm_bindgen(method, getter, js_name = latinAlphabet)]
    fn latin_alphabet(this: &Opts) -> Option<String>;
    #[wasm_bindgen(method, getter, js_name = foreignGuard)]
    fn foreign_guard(this: &Opts) -> Option<bool>;
    #[wasm_bindgen(method, getter, js_name = romanNumerals)]
    fn roman_numerals(this: &Opts) -> Option<bool>;
    #[wasm_bindgen(method, getter, js_name = langPolicy)]
    fn lang_policy(this: &Opts) -> Option<String>;

    #[wasm_bindgen(typescript_type = "Run[]")]
    pub type Runs;
    #[wasm_bindgen(typescript_type = "Run")]
    pub type RunObj;
    #[wasm_bindgen(method, getter)]
    fn text(this: &RunObj) -> Option<String>;
    #[wasm_bindgen(method, getter)]
    fn lang(this: &RunObj) -> Option<String>;
}

fn err(what: &str, got: &str) -> JsError {
    JsError::new(&format!("uz-translit: invalid {what}: {got:?}"))
}

fn options(o: &Option<Opts>) -> Result<(Options, LangPolicy), JsError> {
    let mut opts = Options::default();
    let mut policy = LangPolicy::default();
    let Some(o) = o.as_ref().filter(|o| !o.is_null() && !o.is_undefined()) else {
        return Ok((opts, policy));
    };
    match o.apostrophe().as_deref() {
        None | Some("official") => {}
        Some("ascii") => opts.apostrophe = Apostrophe::Ascii,
        Some(s) => return Err(err("apostrophe", s)),
    }
    match o.latin_alphabet().as_deref() {
        None | Some("1995") => {}
        Some("2026") => opts.latin_alphabet = LatinAlphabet::Y2026,
        Some("auto") => opts.latin_alphabet = LatinAlphabet::Auto,
        Some(s) => return Err(err("latinAlphabet", s)),
    }
    opts.foreign_latin_guard = o.foreign_guard().unwrap_or(opts.foreign_latin_guard);
    opts.roman_numerals = o.roman_numerals().unwrap_or(opts.roman_numerals);
    match o.lang_policy().as_deref() {
        None | Some("convert-ru-en") => {}
        Some("skip-ru-en") => policy = LangPolicy::SkipRuEn,
        Some(s) => return Err(err("langPolicy", s)),
    }
    Ok((opts, policy))
}

fn direction(to: &str) -> Result<Direction, JsError> {
    match to {
        "cyrl" => Ok(Direction::ToCyrillic),
        "latn" => Ok(Direction::ToLatin),
        s => Err(err("direction (want \"cyrl\" or \"latn\")", s)),
    }
}

/// Latin to Cyrillic.
#[wasm_bindgen(js_name = toCyrillic)]
pub fn to_cyrillic(text: &str, opts: Option<Opts>) -> Result<String, JsError> {
    Ok(uz_translit::convert(
        text,
        Direction::ToCyrillic,
        &options(&opts)?.0,
    ))
}

/// Cyrillic to Latin.
#[wasm_bindgen(js_name = toLatin)]
pub fn to_latin(text: &str, opts: Option<Opts>) -> Result<String, JsError> {
    Ok(uz_translit::convert(
        text,
        Direction::ToLatin,
        &options(&opts)?.0,
    ))
}

/// `to` is the target script, "cyrl" or "latn".
#[wasm_bindgen]
pub fn convert(
    text: &str,
    #[wasm_bindgen(unchecked_param_type = "Direction")] to: &str,
    opts: Option<Opts>,
) -> Result<String, JsError> {
    Ok(uz_translit::convert(
        text,
        direction(to)?,
        &options(&opts)?.0,
    ))
}

/// The runs of ONE paragraph, converted as one text; returns each run's new text (same length as the input).
/// Concatenating the result equals `convert` of the concatenated input, for every split.
#[wasm_bindgen(js_name = convertRuns, unchecked_return_type = "string[]")]
pub fn convert_runs(
    #[wasm_bindgen(unchecked_param_type = "Run[]")] runs: Vec<JsValue>,
    #[wasm_bindgen(unchecked_param_type = "Direction")] to: &str,
    opts: Option<Opts>,
) -> Result<Vec<String>, JsError> {
    let dir = direction(to)?;
    let (o, policy) = options(&opts)?;
    let parsed: Vec<(String, bool)> = runs
        .into_iter()
        .map(|v| match v.as_string() {
            Some(s) => Ok((s, true)),
            None => {
                let r: RunObj = v.unchecked_into();
                let text = r
                    .text()
                    .ok_or_else(|| JsError::new("uz-translit: run needs a string or {text}"))?;
                let conv = uz_translit::lang_converts(r.lang().as_deref(), policy);
                Ok((text, conv))
            }
        })
        .collect::<Result<_, JsError>>()?;
    let rs: Vec<Run> = parsed
        .iter()
        .map(|(t, c)| Run {
            text: t,
            convert: *c,
        })
        .collect();
    Ok(uz_translit::convert_runs(&rs, dir, &o)
        .into_iter()
        .map(|c| c.into_owned())
        .collect())
}

/// The dominant script: "latn", "cyrl", or undefined (no letters or a tie).
#[wasm_bindgen(unchecked_return_type = "\"latn\" | \"cyrl\" | undefined")]
pub fn detect(text: &str) -> Option<String> {
    uz_translit::detect(text).map(|s| match s {
        Script::Latin => "latn".into(),
        Script::Cyrillic => "cyrl".into(),
    })
}

/// Package (crate) version.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").into()
}

/// Rules version; store it next to converted text to know which rules produced it.
#[wasm_bindgen(js_name = rulesVersion)]
pub fn rules_version() -> String {
    uz_translit::RULES_VERSION.into()
}
