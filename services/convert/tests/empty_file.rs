//! WP9 fix 2: a 0-byte file is rejected (exit 9) with a clear message whatever its extension claims, and is never converted as an
//! empty text file. Hermetic: the tools are the fake one (they must not even be started).
use convert::exec::{Config, Exit};
use convert::job::{self, Dest};
use convert::plan::{Options, Target, ToolId};

#[test]
fn an_empty_file_is_rejected_whatever_it_claims_to_be() {
    let dir = std::env::temp_dir().join(format!("convert-empty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut cfg = Config::from_env();
    cfg.work_dir = dir.join("work");
    for t in ToolId::ALL {
        cfg.tools
            .insert(t, env!("CARGO_BIN_EXE_convert-fake-tool").into());
    }
    let dest = Dest {
        out_dir: dir.join("dest"),
        out_file: None,
    };
    for (name, to) in [
        ("a.txt", Target::Pdf),
        ("a.docx", Target::Pdf),
        ("a.pdf", Target::Txt),
        ("a.pdf", Target::Docx),
        ("a.xlsx", Target::Csv),
        ("a.csv", Target::Pdf),
        ("a.md", Target::Pdf),
        ("a", Target::Txt),
    ] {
        let p = dir.join(name);
        std::fs::write(&p, b"").unwrap();
        let r = job::convert_file(&cfg, &p, to, &Options::default(), &dest);
        assert_eq!(
            (r.code, r.status),
            (Exit::Rejected.code(), "failed"),
            "{name} to {}: {}",
            to.ext(),
            r.message
        );
        assert!(r.message.contains("empty"), "{name}: {}", r.message);
        assert!(
            r.outputs.is_empty() && r.steps.is_empty(),
            "{name}: a tool ran"
        );
    }
    // a file with one byte is not empty
    let p = dir.join("b.txt");
    std::fs::write(&p, b"x").unwrap();
    assert_eq!(
        job::convert_file(&cfg, &p, Target::Pdf, &Options::default(), &dest).code,
        0
    );
    let _ = std::fs::remove_dir_all(&dir);
}
