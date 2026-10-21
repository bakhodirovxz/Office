//! WP7 review M1 and m12 in the converter core: a scan made earlier is reused when (and only when) the staged copy has the same
//! SHA-256, the inflate budget is charged across files, and a cancelled job stops before it scans.
use convert::exec::Config;
use convert::job::{self, Dest, JobMeta};
use convert::plan::{Options, Target, ToolId};
use convert::scan::{self, Scan, Verdict};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

struct Sandbox(PathBuf);
impl Sandbox {
    fn new(name: &str) -> Sandbox {
        let p = std::env::temp_dir().join(format!("convert-verdict-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Sandbox(p)
    }
    fn cfg(&self) -> Config {
        let mut c = Config::from_env();
        c.work_dir = self.0.join("work");
        for t in ToolId::ALL {
            c.tools
                .insert(t, env!("CARGO_BIN_EXE_convert-fake-tool").into());
        }
        c.sample_ms = 50;
        c
    }
    fn docx(&self) -> PathBuf {
        let p = self.0.join("a.docx");
        let mut w = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
        w.start_file(
            "word/document.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        w.write_all(b"<w:document/>").unwrap();
        w.finish().unwrap();
        p
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(s: &Sandbox, input: &Path, meta: &JobMeta) -> job::FileReport {
    let dest = Dest {
        out_dir: s.0.join("dest"),
        out_file: None,
    };
    job::convert_source(
        &s.cfg(),
        &input.to_path_buf(),
        Target::Pdf,
        &Options::default(),
        &dest,
        meta,
    )
}

#[test]
fn a_verdict_with_the_same_hash_replaces_the_second_scan_and_a_different_one_does_not() {
    let s = Sandbox::new("reuse");
    let docx = s.docx();
    let hash = scan::sha256_file(&docx).unwrap();
    // proof that the verdict is what was used: it claims a macro project, which only a verdict (not the real scan) can say
    let claim = Scan {
        inspected: true,
        features: [("macros", 1u32)].into_iter().collect(),
    };
    let same = JobMeta {
        verdict: Some(Arc::new(Verdict {
            sha256: hash,
            scan: claim.clone(),
        })),
        ..JobMeta::default()
    };
    let r = run(&s, &docx, &same);
    assert_eq!(r.code, 0, "{}", r.message);
    assert!(
        r.warnings.iter().any(|w| w.code == "macros_dropped"),
        "{:?}",
        r.warnings
    );
    // another content hash: the verdict is ignored and the file is scanned for real
    let other = JobMeta {
        verdict: Some(Arc::new(Verdict {
            sha256: "0".repeat(64),
            scan: claim,
        })),
        ..JobMeta::default()
    };
    let r = run(&s, &docx, &other);
    assert_eq!(r.code, 0, "{}", r.message);
    assert!(
        !r.warnings.iter().any(|w| w.code == "macros_dropped"),
        "{:?}",
        r.warnings
    );
    // and with no verdict at all, as before
    assert!(run(&s, &docx, &JobMeta::default()).warnings.is_empty());
}

#[test]
fn the_inflate_budget_is_charged_across_files() {
    let s = Sandbox::new("budget");
    let mk = |name: &str| {
        let p = s.0.join(name);
        let mut w = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
        // five parts just under the 1 MiB ratio floor, so only the budget can stop them
        for i in 0..5 {
            w.start_file(
                format!("p{i}.xml"),
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
            w.write_all(&vec![0u8; 1000 * 1000]).unwrap();
        }
        w.finish().unwrap();
        p
    };
    let (a, b) = (mk("a.docx"), mk("b.docx"));
    let mut budget = 7_000_000u64; // one file costs 5 MB
    assert!(scan::scan_with(&a, &mut budget).is_ok());
    assert_eq!(budget, 2_000_000);
    let e = scan::scan_with(&b, &mut budget).unwrap_err();
    assert!(e.msg.contains("budget"), "{}", e.msg);
    assert_eq!(
        budget, 0,
        "once overdrawn, every later file of the request is refused too"
    );
    assert!(scan::scan_with(&a, &mut budget).is_err());
}

#[test]
fn a_cancelled_job_stops_before_it_scans() {
    let s = Sandbox::new("cancel");
    let docx = s.docx();
    let flag = Arc::new(AtomicBool::new(true));
    let r = run(
        &s,
        &docx,
        &JobMeta {
            cancel: Some(flag),
            ..JobMeta::default()
        },
    );
    assert_eq!(r.code, 4, "{}", r.message);
    assert!(r.message.contains("cancelled"), "{}", r.message);
    assert!(r.steps.is_empty());
}
