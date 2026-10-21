//! `convert formats [--json]`: the matrix as the planner knows it. The API and the UI read this.
use crate::kind::Kind;
use crate::plan::{self, Target};
use serde_json::{Value, json};

pub fn formats_json() -> Value {
    let rows: Vec<Value> = plan::rows()
        .into_iter()
        .map(|(k, t)| json!({"from": k.ext(), "to": t.ext(), "options": plan::row_options(k, t)}))
        .collect();
    let no_route: Vec<&str> = Kind::ALL.into_iter().filter(|k| !rows_has(&rows, *k)).map(Kind::ext).collect();
    json!({
        "inputs": Kind::ALL.iter().map(|k| k.ext()).collect::<Vec<_>>(),
        "targets": Target::ALL.iter().map(|t| t.ext()).collect::<Vec<_>>(),
        "rows": rows,
        "unsupported_inputs": no_route,
    })
}

fn rows_has(rows: &[Value], k: Kind) -> bool {
    rows.iter().any(|r| r["from"] == k.ext())
}

pub fn formats_text() -> String {
    let mut s = String::new();
    for k in Kind::ALL {
        let to: Vec<&str> = Target::ALL.into_iter().filter(|t| plan::rows().contains(&(k, *t))).map(Target::ext).collect();
        s.push_str(&format!("{:<5} -> {}\n", k.ext(), if to.is_empty() { "(unsupported: re-save as .pptx)".into() } else { to.join(" ") }));
    }
    s
}
