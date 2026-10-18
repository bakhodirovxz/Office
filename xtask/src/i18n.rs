//! `cargo xt i18n ...`: canonical catalogs, extraction, export and the owner review pipeline
//! (docs/phase1/designs/i18n.md section 6, WP9b).

use std::fs;
use std::path::{Path, PathBuf};

mod catalog;
mod export;
mod extract;
mod glossary;
mod lex;
mod lint;
mod review;

use catalog::{APPS, Catalog, Mode, Policy};

pub const USAGE: &str = "usage: cargo xt i18n <command>\n  extract [--dry-run] [--date YYYY-MM-DD] [--refresh-where]\n  check-stale\n  export <app|all> [--mode review|release] [--out DIR] [--with-ru]\n  roundtrip [--out DIR]\n  review-export --batch N [--pass both|latn|cyrl] [--size N | --sizes N,N,...] [--out FILE]\n  review-import <csv> [--date YYYY-MM-DD]\n  coverage\n  check";

pub fn run(root: &Path, args: &[&str]) -> Result<String, String> {
    let dir = root.join("i18n");
    let flag = |name: &str| {
        args.iter()
            .position(|a| *a == name)
            .and_then(|i| args.get(i + 1))
            .copied()
    };
    let has = |name: &str| args.contains(&name);
    match args {
        ["extract", ..] => {
            let unis = extract::extract_all(root);
            let date = flag("--date").map(String::from).unwrap_or_else(today);
            let mut cat = Catalog::load(&dir)?;
            let rep = extract::merge_report(&mut cat, &unis, &date, has("--refresh-where"));
            if has("--dry-run") {
                return Ok(format!("{}{}", extract::report(&unis, None), rep.text()));
            }
            cat.save(&dir)?;
            Ok(format!(
                "{}{}written to {}",
                extract::report(&unis, Some(&cat)),
                rep.text(),
                dir.display()
            ))
        }
        ["check-stale", ..] => {
            // The sync gate (scripts/sync-upstream.sh): is our catalog current with the upstream catalogs in upstream/?
            let unis = extract::extract_all(root);
            let mut cat = Catalog::load(&dir)?;
            let rep = extract::merge_report(&mut cat, &unis, &today(), false);
            if rep.is_clean() {
                Ok("ok: i18n catalogs are current with upstream".into())
            } else {
                Err(format!(
                    "i18n catalogs are STALE: an upstream catalog changed.\n{}run `cargo xt i18n extract` (scripts/sync-upstream.sh --apply does it) and commit i18n/*.tsv",
                    rep.text()
                ))
            }
        }
        ["export", app, ..] => {
            let mode = match flag("--mode").unwrap_or("review") {
                "review" => Mode::Review,
                "release" => Mode::Release,
                o => return Err(format!("unknown mode {o:?}")),
            };
            let cat = Catalog::load(&dir)?;
            let chains = export::Chains::load(&dir)?;
            let policy = Policy::load(&dir)?;
            let out = match flag("--out") {
                Some(o) => PathBuf::from(o),
                None => export::overlay_root()?,
            };
            let apps: Vec<&str> = if *app == "all" {
                APPS.to_vec()
            } else {
                vec![*app]
            };
            let mut lines = vec![format!("export ({}):", policy.describe())];
            for a in apps {
                if !APPS.contains(&a) {
                    return Err(format!("unknown app {a:?}"));
                }
                lines.extend(export::export_app(
                    &cat,
                    a,
                    mode,
                    &chains,
                    &out,
                    has("--with-ru"),
                    &policy,
                )?);
            }
            Ok(lines.join("\n"))
        }
        ["roundtrip", ..] => {
            let cat = Catalog::load(&dir)?;
            let chains = export::Chains::load(&dir)?;
            let out = match flag("--out") {
                Some(o) => PathBuf::from(o),
                None => export::overlay_root()?,
            };
            export::roundtrip(root, &cat, &chains, &out).map(|l| l.join("\n"))
        }
        ["review-export", ..] => {
            let batch: usize = flag("--batch")
                .ok_or("--batch N is required")?
                .parse()
                .map_err(|_| "--batch must be a number")?;
            let pass = review::Pass::parse(flag("--pass").unwrap_or("both"))?;
            let sizes: Vec<usize> = match flag("--sizes") {
                Some(l) => l
                    .split(',')
                    .map(|n| {
                        n.trim()
                            .parse::<usize>()
                            .map_err(|_| "--sizes must be numbers like 400,660,660".to_string())
                    })
                    .collect::<Result<_, _>>()?,
                None => vec![
                    flag("--size")
                        .map_or(Ok(400), str::parse)
                        .map_err(|_| "--size must be a number")?,
                ],
            };
            let cat = Catalog::load(&dir)?;
            let terms = glossary::terms_by_row(&cat, &glossary::load_index(&dir)?);
            let (csv, n) = review::review_export_with(&cat, batch, pass, &sizes, &terms)?;
            let out = flag("--out")
                .map(PathBuf::from)
                .unwrap_or_else(|| dir.join("review").join(format!("batch-{batch:02}.csv")));
            if let Some(p) = out.parent() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            fs::write(&out, csv).map_err(|e| format!("{}: {e}", out.display()))?;
            Ok(format!(
                "batch {batch} ({}): {n} rows -> {}",
                pass.name(),
                out.display()
            ))
        }
        ["review-import", file, ..] => {
            let mut cat = Catalog::load(&dir)?;
            let date = flag("--date").map(String::from).unwrap_or_else(today);
            let text = fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
            let st = review::review_import(&mut cat, &text, &date)?;
            cat.save(&dir)?;
            let name = Path::new(file)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file);
            let line = review::log_line(name, &st, &date);
            let log = dir.join("review/LOG.md");
            if let Some(p) = log.parent() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut old = fs::read_to_string(&log).unwrap_or_else(|_| "# Review log\n\nOne line per import, appended by `cargo xt i18n review-import`.\n\n".into());
            old.push_str(&line);
            fs::write(&log, old).map_err(|e| e.to_string())?;
            Ok(line.trim_end().to_string())
        }
        ["coverage", ..] => {
            let cat = Catalog::load(&dir)?;
            Ok(format!(
                "{}\n{}",
                review::coverage(&cat),
                lint::review_summary(&cat, &Policy::load(&dir)?)
            ))
        }
        ["check", ..] => {
            let cat = Catalog::load(&dir)?;
            let bad = review::check_ids(&cat);
            if bad.is_empty() {
                Ok(format!(
                    "ok: {} rows, all ids match",
                    cat.scopes.values().map(Vec::len).sum::<usize>()
                ))
            } else {
                Err(bad.join("\n"))
            }
        }
        ["lint", ..] => {
            let cat = Catalog::load(&dir)?;
            let findings = lint::lint(&cat);
            let rows = lint::rows_with_uzbek(&cat);
            let review = lint::review_summary(&cat, &Policy::load(&dir)?);
            if let Some(path) = flag("--report") {
                fs::write(path, lint::report_with(&findings, rows, &review))
                    .map_err(|e| format!("{path}: {e}"))?;
            }
            let errors = findings
                .iter()
                .filter(|f| f.level == lint::Level::Error)
                .count();
            let text = format!(
                "i18n lint: {rows} rows with an Uzbek draft, {errors} error(s), {} warning(s)
{}",
                findings.len() - errors,
                lint::summary(&findings) + &review
            );
            if errors == 0 {
                Ok(text.trim_end().to_string())
            } else {
                let mut detail = String::new();
                for f in findings
                    .iter()
                    .filter(|f| f.level == lint::Level::Error)
                    .take(40)
                {
                    detail.push_str(&format!(
                        "  {} {}/{} {:?}: {}
",
                        f.code, f.scope, f.id, f.source, f.detail
                    ));
                }
                Err(format!("{text}{detail}"))
            }
        }
        ["glossary-scan", ..] => {
            let cat = Catalog::load(&dir)?;
            let uses = glossary::scan(&cat);
            fs::write(dir.join(glossary::INDEX_FILE), glossary::write_index(&uses))
                .map_err(|e| e.to_string())?;
            let rows: std::collections::BTreeSet<_> =
                uses.iter().map(|u| (&u.scope, &u.id)).collect();
            Ok(format!(
                "glossary-scan: {} use(s) in {} string(s) -> {}",
                uses.len(),
                rows.len(),
                dir.join(glossary::INDEX_FILE).display()
            ))
        }
        ["glossary-apply", ..] => {
            let mut cat = Catalog::load(&dir)?;
            let mut uses = glossary::load_index(&dir)?;
            if uses.is_empty() {
                return Err("i18n/glossary-use.tsv is missing or empty: run `cargo xt i18n glossary-scan` first".into());
            }
            let date = flag("--date").map(String::from).unwrap_or_else(today);
            let rep = glossary::apply(&mut cat, &mut uses, &date);
            let write = has("--write");
            if write {
                cat.save(&dir)?;
                fs::write(dir.join(glossary::INDEX_FILE), glossary::write_index(&uses))
                    .map_err(|e| e.to_string())?;
            }
            Ok(rep.text(write).trim_end().to_string())
        }
        _ => Err(USAGE.to_string()),
    }
}

/// Today's UTC date as YYYY-MM-DD (civil-from-days, no date crate).
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    #[test]
    fn today_is_a_plausible_date() {
        let t = super::today();
        assert_eq!(t.len(), 10);
        assert!(t.starts_with("20"));
    }
}
