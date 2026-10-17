//! A small Rust lexer: just enough to find string literals, idents and punctuation in upstream
//! sources (comments skipped, escapes decoded). Not a parser; it never fails.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    Ident(String),
    Punct(char),
    Lit(String),
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
}

impl Token {
    pub fn lit(&self) -> Option<&str> {
        match &self.tok {
            Tok::Lit(s) => Some(s),
            _ => None,
        }
    }
    pub fn is_punct(&self, c: char) -> bool {
        self.tok == Tok::Punct(c)
    }
    pub fn is_ident(&self, name: &str) -> bool {
        matches!(&self.tok, Tok::Ident(i) if i == name)
    }
}

pub fn lex(src: &str) -> Vec<Token> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line) = (0usize, 1usize);
    while i < c.len() {
        let ch = c[i];
        if ch == '\n' {
            line += 1;
            i += 1;
        } else if ch.is_whitespace() {
            i += 1;
        } else if ch == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if ch == '/' && c.get(i + 1) == Some(&'*') {
            let mut depth = 1;
            i += 2;
            while i < c.len() && depth > 0 {
                if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                } else {
                    if c[i] == '\n' {
                        line += 1;
                    }
                    i += 1;
                }
            }
        } else if ch == 'r'
            && matches!(c.get(i + 1), Some('"') | Some('#'))
            && raw_start(&c, i + 1).is_some()
        {
            let (hashes, body) = raw_start(&c, i + 1).unwrap_or((0, i + 1));
            let start_line = line;
            let mut j = body;
            let mut text = String::new();
            while j < c.len() {
                if c[j] == '"' && (0..hashes).all(|k| c.get(j + 1 + k) == Some(&'#')) {
                    j += 1 + hashes;
                    break;
                }
                if c[j] == '\n' {
                    line += 1;
                }
                text.push(c[j]);
                j += 1;
            }
            out.push(Token {
                tok: Tok::Lit(text),
                line: start_line,
            });
            i = j;
        } else if ch == '"' {
            let start_line = line;
            let mut text = String::new();
            i += 1;
            while i < c.len() && c[i] != '"' {
                if c[i] == '\\' {
                    i += 1;
                    match c.get(i) {
                        Some('n') => text.push('\n'),
                        Some('r') => text.push('\r'),
                        Some('t') => text.push('\t'),
                        Some('0') => text.push('\0'),
                        Some('x') => {
                            let hex: String = c[i + 1..(i + 3).min(c.len())].iter().collect();
                            if let Some(ch) =
                                u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)
                            {
                                text.push(ch);
                            }
                            i += 2;
                        }
                        Some('u') => {
                            let mut j = i + 1;
                            let mut hex = String::new();
                            if c.get(j) == Some(&'{') {
                                j += 1;
                                while j < c.len() && c[j] != '}' {
                                    if c[j] != '_' {
                                        hex.push(c[j]);
                                    }
                                    j += 1;
                                }
                            }
                            if let Some(ch) =
                                u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)
                            {
                                text.push(ch);
                            }
                            i = j;
                        }
                        Some('\n') => {
                            line += 1;
                            while i + 1 < c.len() && c[i + 1].is_whitespace() {
                                i += 1;
                                if c[i] == '\n' {
                                    line += 1;
                                }
                            }
                        }
                        Some(other) => text.push(*other), // \\ \" \'
                        None => {}
                    }
                    i += 1;
                } else {
                    if c[i] == '\n' {
                        line += 1;
                    }
                    text.push(c[i]);
                    i += 1;
                }
            }
            i += 1;
            out.push(Token {
                tok: Tok::Lit(text),
                line: start_line,
            });
        } else if ch == '\'' {
            if c.get(i + 1) == Some(&'\\') {
                i += 2;
                while i < c.len() && c[i] != '\'' {
                    i += 1;
                }
                i += 1;
            } else if c.get(i + 2) == Some(&'\'') {
                i += 3;
            } else {
                i += 1;
                while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
                    i += 1;
                }
            }
        } else if ch.is_alphanumeric() || ch == '_' {
            let s = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            out.push(Token {
                tok: Tok::Ident(c[s..i].iter().collect()),
                line,
            });
        } else {
            out.push(Token {
                tok: Tok::Punct(ch),
                line,
            });
            i += 1;
        }
    }
    out
}

/// At `i` (after an `r`): `#`* then `"`. Returns (hash count, index of the first body char).
fn raw_start(c: &[char], mut i: usize) -> Option<(usize, usize)> {
    let mut hashes = 0;
    while c.get(i) == Some(&'#') {
        hashes += 1;
        i += 1;
    }
    (c.get(i) == Some(&'"')).then_some((hashes, i + 1))
}

/// Remove the token ranges of `#[cfg(test)] mod name { ... }` blocks and of `#[...]` attributes.
pub fn strip_tests_and_attrs(toks: Vec<Token>) -> Vec<Token> {
    let mut out = Vec::with_capacity(toks.len());
    let mut i = 0;
    while i < toks.len() {
        if toks[i].is_punct('#') && toks.get(i + 1).is_some_and(|t| t.is_punct('[')) {
            let mut j = i + 2;
            let mut depth = 1;
            while j < toks.len() && depth > 0 {
                if toks[j].is_punct('[') {
                    depth += 1;
                } else if toks[j].is_punct(']') {
                    depth -= 1;
                }
                j += 1;
            }
            let attr = &toks[i + 2..j.saturating_sub(1)];
            let is_cfg_test =
                attr.len() == 4 && attr[0].is_ident("cfg") && attr[2].is_ident("test");
            if is_cfg_test && toks.get(j).is_some_and(|t| t.is_ident("mod")) {
                let mut k = j;
                while k < toks.len() && !toks[k].is_punct('{') && !toks[k].is_punct(';') {
                    k += 1;
                }
                if toks.get(k).is_some_and(|t| t.is_punct('{')) {
                    let mut depth = 0;
                    while k < toks.len() {
                        if toks[k].is_punct('{') {
                            depth += 1;
                        } else if toks[k].is_punct('}') {
                            depth -= 1;
                            if depth == 0 {
                                k += 1;
                                break;
                            }
                        }
                        k += 1;
                    }
                }
                i = k;
                continue;
            }
            i = j;
            continue;
        }
        out.push(toks[i].clone());
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lits(src: &str) -> Vec<String> {
        lex(src)
            .iter()
            .filter_map(|t| t.lit().map(str::to_string))
            .collect()
    }

    #[test]
    fn decodes_escapes_and_skips_comments() {
        let src =
            "// \"no\"\nlet a = \"Top\\nAlign\"; /* \"no\" */ let b = \"q\\\"x\\u{2026}\\\\\";";
        assert_eq!(lits(src), vec!["Top\nAlign", "q\"x\u{2026}\\"]);
    }

    #[test]
    fn raw_strings_chars_and_lifetimes() {
        let src =
            "fn f<'a>(x: &'a str) { let c = '\"'; let r = r#\"a \"b\"\"#; let d = '\\''; \"z\" }";
        assert_eq!(lits(src), vec!["a \"b\"", "z"]);
    }

    #[test]
    fn strips_cfg_test_modules() {
        let src = "fn a() { \"keep\" } #[cfg(test)] mod tests { fn b() { \"drop\" } } #[doc = \"attr\"] fn c() { \"keep2\" }";
        let toks = strip_tests_and_attrs(lex(src));
        let l: Vec<_> = toks.iter().filter_map(|t| t.lit()).collect();
        assert_eq!(l, vec!["keep", "keep2"]);
    }
}
