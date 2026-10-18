//! `cargo xt gen-domain` and `cargo xt lockdiff` (PLAN WP3): generate a build domain that wraps the
//! unmodified upstream app sources, and check that its lock matches upstream's.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use toml_edit::{Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, Value};

type Res<T> = Result<T, String>;

const KINDS: [&str; 3] = ["desktop", "web", "cli"];

struct Domain {
    name: String,
    upstream: String,
    /// kind -> (upstream app dir name, our bin name)
    apps: BTreeMap<String, (String, String)>,
}

fn read_doc(path: &Path) -> Res<DocumentMut> {
    fs::read_to_string(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .parse::<DocumentMut>()
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn load_domain(root: &Path, name: &str) -> Res<Domain> {
    let doc = read_doc(&root.join("domains").join(name).join("domain.toml"))?;
    let upstream = doc
        .get("upstream")
        .and_then(Item::as_str)
        .ok_or("domain.toml: missing `upstream`")?
        .to_string();
    let mut apps = BTreeMap::new();
    for kind in KINDS {
        let t = doc
            .get(kind)
            .ok_or(format!("domain.toml: missing [{kind}]"))?;
        let src = t
            .get("src")
            .and_then(Item::as_str)
            .ok_or(format!("domain.toml [{kind}]: missing src"))?;
        let bin = t
            .get("bin")
            .and_then(Item::as_str)
            .ok_or(format!("domain.toml [{kind}]: missing bin"))?;
        apps.insert(kind.to_string(), (src.to_string(), bin.to_string()));
    }
    Ok(Domain {
        name: name.to_string(),
        upstream,
        apps,
    })
}

fn domain_names(root: &Path) -> Res<Vec<String>> {
    let mut v = Vec::new();
    for e in fs::read_dir(root.join("domains"))
        .map_err(|e| format!("domains: {e}"))?
        .flatten()
    {
        if e.path().join("domain.toml").exists() {
            v.push(e.file_name().to_string_lossy().to_string());
        }
    }
    v.sort();
    Ok(v)
}

/// `cargo xt domain-upstream <d>`
pub fn domain_upstream(root: &Path, d: &str) -> Res<String> {
    Ok(load_domain(root, d)?.upstream)
}

fn base_sha7(root: &Path, upstream: &str) -> String {
    let text =
        fs::read_to_string(root.join("patches").join(upstream).join("BASE")).unwrap_or_default();
    text.lines()
        .find_map(|l| l.strip_prefix("sha="))
        .map(|s| s.chars().take(7).collect())
        .unwrap_or_else(|| "unknown".into())
}

/// Normalise `a/b/../c` without touching the file system.
fn normalize(p: &str) -> Res<String> {
    let mut out: Vec<&str> = Vec::new();
    for c in p.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                if out.pop().is_none() {
                    return Err(format!("path '{p}' leaves the upstream tree"));
                }
            }
            c => out.push(c),
        }
    }
    Ok(out.join("/"))
}

/// A path relative to the upstream root -> a path from a manifest `depth` levels below the domain
/// root's parent, pointing into `upstream/<name>/`.
fn upstream_path(depth: usize, upstream: &str, rel_to_upstream_root: &str) -> Res<String> {
    let rel = normalize(rel_to_upstream_root)?;
    Ok(format!("{}upstream/{upstream}/{rel}", "../".repeat(depth)))
}

fn as_inline(item: &Item) -> Res<InlineTable> {
    match item {
        Item::Value(Value::String(s)) => {
            let mut t = InlineTable::new();
            t.insert("version", s.value().as_str().into());
            Ok(t)
        }
        Item::Value(Value::InlineTable(t)) => Ok(t.clone()),
        Item::Table(t) => Ok(t.clone().into_inline_table()),
        other => Err(format!("unsupported dependency form: {other}")),
    }
}

struct Ctx<'a> {
    upstream: &'a str,
    ws_deps: &'a Table,
    /// directory of the upstream app manifest, relative to the upstream root (e.g. `apps/gridcraft`)
    app_dir: String,
    /// levels from our manifest up to the repository root (apps/<pkg> inside domains/<d> -> 4)
    depth: usize,
}

fn resolve_dep(ctx: &Ctx, key: &str, local: &Item) -> Res<Item> {
    let mut t = as_inline(local)?;
    let is_ws = t.get("workspace").and_then(Value::as_bool).unwrap_or(false);
    let mut out;
    if is_ws {
        let ws = ctx.ws_deps.get(key).ok_or(format!(
            "dependency '{key}': not in [workspace.dependencies]"
        ))?;
        out = as_inline(ws)?;
        if let Some(path) = out.get("path").and_then(Value::as_str).map(str::to_string) {
            out.insert(
                "path",
                upstream_path(ctx.depth, ctx.upstream, &path)?.into(),
            );
        }
        if let Some(extra) = t.get("features").and_then(Value::as_array).cloned() {
            let mut feats = out
                .get("features")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let have: BTreeSet<String> = feats
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            for f in extra.iter().filter_map(Value::as_str) {
                if !have.contains(f) {
                    feats.push(f);
                }
            }
            out.insert("features", Value::Array(feats));
        }
        for k in ["optional", "default-features", "package"] {
            if let Some(v) = t.remove(k) {
                out.insert(k, v);
            }
        }
    } else {
        if let Some(path) = t.get("path").and_then(Value::as_str).map(str::to_string) {
            let joined = format!("{}/{}", ctx.app_dir, path);
            t.insert(
                "path",
                upstream_path(ctx.depth, ctx.upstream, &joined)?.into(),
            );
        }
        out = t;
    }
    // A bare `{ version = "x" }` is written back as a string.
    if out.len() == 1
        && let Some(v) = out.get("version").and_then(Value::as_str)
    {
        return Ok(Item::Value(v.into()));
    }
    Ok(Item::Value(Value::InlineTable(out)))
}

fn resolve_table(ctx: &Ctx, src: &Table) -> Res<Table> {
    let mut out = Table::new();
    for (k, v) in src.iter() {
        out.insert(k, resolve_dep(ctx, k, v)?);
    }
    Ok(out)
}

const HEADER: &str = "# GENERATED by cargo xt gen-domain";

fn header(d: &Domain, sha7: &str) -> String {
    format!(
        "{HEADER} {} from upstream/{}@{sha7}; edit domain.toml instead\n",
        d.name, d.upstream
    )
}

fn ws_true() -> Item {
    let mut t = InlineTable::new();
    t.insert("workspace", true.into());
    Item::Value(Value::InlineTable(t))
}

fn gen_app(
    root: &Path,
    d: &Domain,
    kind: &str,
    sha7: &str,
    ws_deps: &Table,
) -> Res<(PathBuf, String)> {
    let (src, bin) = &d.apps[kind];
    let pkg = format!("{}-{kind}", d.name);
    let up_root = root.join("upstream").join(&d.upstream);
    let up_doc = read_doc(&up_root.join("apps").join(src).join("Cargo.toml"))?;
    let ctx = Ctx {
        upstream: &d.upstream,
        ws_deps,
        app_dir: format!("apps/{src}"),
        depth: 4,
    };

    let mut doc = DocumentMut::new();
    let up_pkg = up_doc["package"]
        .as_table()
        .ok_or("upstream app: no [package]")?;
    let mut p = Table::new();
    p.insert("name", pkg.as_str().into());
    for k in ["version", "edition", "license", "rust-version"] {
        p.insert(k, ws_true());
    }
    if let Some(desc) = up_pkg.get("description") {
        p.insert("description", desc.clone());
    }
    p.insert("publish", false.into());
    if up_pkg.contains_key("default-run") {
        p.insert("default-run", bin.as_str().into());
    }
    if up_root.join("apps").join(src).join("build.rs").exists() {
        p.insert("build", "build.rs".into());
    }
    doc.insert("package", Item::Table(p));

    if up_doc.contains_key("lints") {
        doc.insert("lints", up_doc["lints"].clone());
    }

    let mut b = Table::new();
    b.insert("name", bin.as_str().into());
    b.insert(
        "path",
        format!("../../../../upstream/{}/apps/{src}/src/main.rs", d.upstream).into(),
    );
    let mut bins = ArrayOfTables::new();
    bins.push(b);
    doc.insert("bin", Item::ArrayOfTables(bins));

    for sect in ["dependencies", "build-dependencies", "dev-dependencies"] {
        if let Some(t) = up_doc.get(sect).and_then(Item::as_table) {
            doc.insert(sect, Item::Table(resolve_table(&ctx, t)?));
        }
    }
    if let Some(targets) = up_doc.get("target").and_then(Item::as_table) {
        let mut out = Table::new();
        out.set_implicit(true);
        for (cfg, tt) in targets.iter() {
            let Some(tt) = tt.as_table() else { continue };
            let mut ot = Table::new();
            for sect in ["dependencies", "build-dependencies", "dev-dependencies"] {
                if let Some(t) = tt.get(sect).and_then(Item::as_table) {
                    ot.insert(sect, Item::Table(resolve_table(&ctx, t)?));
                }
            }
            if !ot.is_empty() {
                ot.set_implicit(true);
                out.insert(cfg, Item::Table(ot));
            }
        }
        if !out.is_empty() {
            doc.insert("target", Item::Table(out));
        }
    }
    if let Some(f) = up_doc.get("features") {
        doc.insert("features", f.clone());
    }
    let path = root
        .join("domains")
        .join(&d.name)
        .join("apps")
        .join(&pkg)
        .join("Cargo.toml");
    Ok((path, format!("{}{}", header(d, sha7), doc)))
}

fn gen_root(root: &Path, d: &Domain, sha7: &str) -> Res<(PathBuf, String, Table)> {
    let up = read_doc(&root.join("upstream").join(&d.upstream).join("Cargo.toml"))?;
    let ws = up["workspace"]
        .as_table()
        .ok_or("upstream root: no [workspace]")?;
    let mut doc = DocumentMut::new();
    let mut w = Table::new();
    let mut members = Array::new();
    members.push("apps/*");
    if root.join("domains").join(&d.name).join("crates").is_dir() {
        members.push("crates/*");
    }
    w.insert("members", Item::Value(Value::Array(members)));
    w.insert("resolver", "3".into());
    let up_pkg = ws
        .get("package")
        .and_then(Item::as_table)
        .ok_or("upstream: no [workspace.package]")?;
    let mut wp = Table::new();
    wp.insert("version", "0.1.0".into());
    wp.insert(
        "edition",
        up_pkg.get("edition").cloned().ok_or("no edition")?,
    );
    wp.insert(
        "license",
        up_pkg.get("license").cloned().ok_or("no license")?,
    );
    wp.insert(
        "rust-version",
        up_pkg
            .get("rust-version")
            .cloned()
            .ok_or("no rust-version")?,
    );
    w.insert("package", Item::Table(wp));
    if let Some(l) = ws.get("lints") {
        w.insert("lints", l.clone());
    }
    doc.insert("workspace", Item::Table(w));
    if let Some(pr) = up.get("profile") {
        doc.insert("profile", pr.clone());
    }
    if let Some(patch) = up.get("patch").and_then(Item::as_table) {
        let mut out = Table::new();
        for (reg, tbl) in patch.iter() {
            let Some(tbl) = tbl.as_table() else { continue };
            let mut ot = Table::new();
            for (k, v) in tbl.iter() {
                let mut t = as_inline(v)?;
                if let Some(path) = t.get("path").and_then(Value::as_str).map(str::to_string) {
                    t.insert("path", upstream_path(2, &d.upstream, &path)?.into());
                }
                ot.insert(k, Item::Value(Value::InlineTable(t)));
            }
            out.insert(reg, Item::Table(ot));
        }
        doc.insert("patch", Item::Table(out));
    }
    let ws_deps = ws
        .get("dependencies")
        .and_then(Item::as_table)
        .cloned()
        .unwrap_or_default();
    Ok((
        root.join("domains").join(&d.name).join("Cargo.toml"),
        format!("{}{}", header(d, sha7), doc),
        ws_deps,
    ))
}

fn generate(root: &Path, name: &str) -> Res<Vec<(PathBuf, String)>> {
    let d = load_domain(root, name)?;
    let sha7 = base_sha7(root, &d.upstream);
    let (rp, rt, ws_deps) = gen_root(root, &d, &sha7)?;
    let mut out = vec![(rp, rt)];
    for kind in KINDS {
        out.push(gen_app(root, &d, kind, &sha7, &ws_deps)?);
    }
    Ok(out)
}

/// `cargo xt gen-domain [--check] <d|all>`
pub fn gen_domain(root: &Path, check: bool, which: &str) -> Res<String> {
    let names = if which == "all" {
        domain_names(root)?
    } else {
        vec![which.to_string()]
    };
    let mut drift = Vec::new();
    for n in &names {
        for (path, text) in generate(root, n)? {
            if check {
                if fs::read_to_string(&path).map(|t| t != text).unwrap_or(true) {
                    drift.push(path.display().to_string());
                }
            } else {
                fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
                fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
            }
        }
    }
    if !drift.is_empty() {
        return Err(format!("drift in:\n  {}", drift.join("\n  ")));
    }
    Ok(if check {
        format!("{} domains up to date", names.len())
    } else {
        format!("generated {} domains", names.len())
    })
}

type Pkgs = BTreeMap<(String, String), BTreeSet<String>>;

fn read_lock(path: &Path) -> Res<Pkgs> {
    let doc = read_doc(path)?;
    let mut m: Pkgs = BTreeMap::new();
    for p in doc
        .get("package")
        .and_then(Item::as_array_of_tables)
        .ok_or("lock: no packages")?
        .iter()
    {
        let name = p
            .get("name")
            .and_then(Item::as_str)
            .unwrap_or_default()
            .to_string();
        let ver = p
            .get("version")
            .and_then(Item::as_str)
            .unwrap_or_default()
            .to_string();
        let src = p
            .get("source")
            .and_then(Item::as_str)
            .unwrap_or("")
            .to_string();
        m.entry((name, src)).or_default().insert(ver);
    }
    Ok(m)
}

/// `cargo xt lockdiff <domain-lock> <upstream-lock>`
pub fn lockdiff(domain: &Path, upstream: &Path) -> Res<String> {
    let (d, u) = (read_lock(domain)?, read_lock(upstream)?);
    let (mut changes, mut removed, mut foreign) = (Vec::new(), 0usize, Vec::new());
    for ((name, src), vers) in &d {
        match u.get(&(name.clone(), src.clone())) {
            Some(uv) => {
                for v in vers.difference(uv) {
                    changes.push(format!(
                        "{name} {v} ({}) not in upstream lock",
                        if src.is_empty() { "path" } else { src }
                    ));
                }
            }
            None if src.is_empty() => {} // a path package of ours
            None => foreign.push(format!("{name} {vers:?} {src}")),
        }
    }
    for (k, uv) in &u {
        match d.get(k) {
            Some(dv) => removed += uv.difference(dv).count(),
            None => removed += uv.len(),
        }
    }
    let line = format!(
        "{} version changes, {removed} removed (dev/xtask), {} foreign additions",
        changes.len(),
        foreign.len()
    );
    if changes.is_empty() && foreign.is_empty() {
        Ok(line)
    } else {
        Err(format!(
            "{line}\n  {}",
            changes
                .into_iter()
                .chain(foreign)
                .collect::<Vec<_>>()
                .join("\n  ")
        ))
    }
}
