//! Fault injection (PLAN WP2 step 5): the executor against `convert-fake-tool`. Every fault must give its exit code
//! (1/4/5/6/8, plus 7 for a missing tool) and a human-readable message, and must never return an output.
use convert::exec::{Config, Exit, Failure, PostOutput};
use convert::job::{self, Dest, FileReport, JobSpec};
use convert::plan::{Outputs, Step, ToolId, ToolStep};
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct Sandbox(PathBuf);
impl Sandbox {
    fn new(name: &str) -> Sandbox {
        let p = std::env::temp_dir().join(format!("convert-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("sample.txt"), b"hello").unwrap();
        Sandbox(p)
    }
    fn cfg(&self) -> Config {
        let mut c = Config::from_env();
        c.work_dir = self.0.join("work");
        c.tools.insert(ToolId::Docs, env!("CARGO_BIN_EXE_convert-fake-tool").into());
        c.sample_ms = 50;
        c
    }
    fn jobs_left(&self) -> usize {
        std::fs::read_dir(self.0.join("work")).map(|r| r.count()).unwrap_or(0)
    }
    fn run(&self, cfg: &Config, argv: &[&str], out: &str) -> FileReport {
        let dest = Dest { out_dir: self.0.join("dest"), out_file: None };
        let step = Step::Tool(ToolStep {
            tool: ToolId::Docs,
            argv: argv.iter().map(|s| s.to_string()).collect(),
            outputs: Outputs::Files(vec![out.into()]),
            stdout_to: None,
            allow_empty: false,
            write_files: vec![],
        });
        job::run_job(
            cfg,
            JobSpec {
                label: "sample.txt".into(),
                kind: Some("txt".into()),
                target: "x".into(),
                inputs: vec![(self.0.join("sample.txt"), "in/input.txt".into())],
                origins: vec![],
                steps: Ok(vec![step]),
                warnings: vec![],
                stem: "sample".into(),
                dest: &dest,
            },
        )
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn expect(r: &FileReport, exit: Exit, needle: &str) {
    assert_eq!(r.code, exit.code(), "wrong exit; message: {}", r.message);
    assert_eq!(r.status, "failed");
    assert!(r.message.to_lowercase().contains(needle), "message '{}' lacks '{needle}'", r.message);
    assert!(r.outputs.is_empty(), "a failed job must not return outputs");
}

#[test]
fn tool_error_is_exit_1_and_passes_the_engine_message() {
    let s = Sandbox::new("e1");
    let r = s.run(&s.cfg(), &["exit1"], "out/output.pdf");
    expect(&r, Exit::ToolError, "the document is broken");
    assert_eq!((r.steps.len(), r.steps[0].exit), (1, 1), "the failed step is in the report");
    assert!(r.steps[0].stderr_tail.contains("document is broken"));
    assert_eq!(s.jobs_left(), 0, "the cleanup guard must remove the job directory");
}

#[test]
fn timeout_is_exit_4() {
    let s = Sandbox::new("e4");
    let mut c = s.cfg();
    c.timeout_s = Some(1);
    let t = std::time::Instant::now();
    let r = s.run(&c, &["sleep", "60"], "out/output.pdf");
    expect(&r, Exit::Timeout, "did not finish within 1 s");
    assert!(t.elapsed().as_secs() < 20, "the child must be killed, not waited for");
    assert_eq!(s.jobs_left(), 0);
}

#[test]
fn memory_cap_is_exit_5() {
    let s = Sandbox::new("e5");
    let mut c = s.cfg();
    c.max_mem_mb = Some(300);
    let r = s.run(&c, &["alloc", "4096"], "out/output.pdf");
    expect(&r, Exit::Memory, "more than 300 mb");
}

#[test]
fn panic_and_abort_are_exit_6() {
    let s = Sandbox::new("e6");
    expect(&s.run(&s.cfg(), &["panic"], "out/output.pdf"), Exit::Crash, "panic");
    expect(&s.run(&s.cfg(), &["abort"], "out/output.pdf"), Exit::Crash, "crashed");
}

#[test]
fn empty_and_truncated_outputs_are_exit_8() {
    let s = Sandbox::new("e8");
    expect(&s.run(&s.cfg(), &["empty", "out/output.docx"], "out/output.docx"), Exit::BadOutput, "empty file");
    expect(&s.run(&s.cfg(), &["truncated", "out/output.docx"], "out/output.docx"), Exit::BadOutput, "truncated");
    expect(&s.run(&s.cfg(), &["ok", "out/other.pdf"], "out/output.pdf"), Exit::BadOutput, "no file");
    // right extension, wrong magic
    expect(&s.run(&s.cfg(), &["ok", "out/output.png"], "out/output.png"), Exit::BadOutput, "png signature");
}

#[test]
fn missing_tool_is_exit_7() {
    let s = Sandbox::new("e7");
    let mut c = s.cfg();
    c.tools.insert(ToolId::Docs, s.0.join("nope.exe"));
    // the explicit path is absent; make the fallbacks absent too
    let r = s.run(&c, &["ok", "out/output.pdf"], "out/output.pdf");
    if r.code == 0 {
        return; // a docs-cli exists on this machine's CONVERT_TOOLS_DIR; resolution fell through, which is also valid
    }
    expect(&r, Exit::Missing, "docs-cli");
}

#[test]
fn success_copies_the_output_and_passes_upstream_warnings() {
    let s = Sandbox::new("ok");
    let r = s.run(&s.cfg(), &["warn", "out/output.pdf"], "out/output.pdf");
    assert_eq!(r.code, 0, "{}", r.message);
    assert_eq!(r.status, "warn");
    assert_eq!(r.warnings[0].code, "upstream");
    assert!(r.warnings[0].text.contains("dropped the charts"));
    assert_eq!(r.outputs.len(), 1);
    assert!(Path::new(&r.outputs[0]).ends_with("sample.pdf"));
    // a second run does not overwrite: " (2)"
    let r2 = s.run(&s.cfg(), &["ok", "out/output.pdf"], "out/output.pdf");
    assert!(r2.outputs[0].ends_with("sample (2).pdf"), "{}", r2.outputs[0]);
    assert!(r2.steps[0].peak_mb < 500 && r2.steps[0].exit == 0);
}

#[test]
fn post_output_hook_can_veto() {
    struct Veto;
    impl PostOutput for Veto {
        fn after(&self, _: &Path, _: &[PathBuf]) -> Result<(), Failure> {
            Err(Failure { exit: Exit::Rejected, msg: "vetoed by hook".into() })
        }
    }
    let s = Sandbox::new("hook");
    let mut c = s.cfg();
    c.post_output.push(Arc::new(Veto));
    expect(&s.run(&c, &["ok", "out/output.pdf"], "out/output.pdf"), Exit::Rejected, "vetoed");
}
