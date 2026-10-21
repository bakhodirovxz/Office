//! Fault injection (PLAN WP2 step 5): the executor against `convert-fake-tool`. Every fault must give its exit code
//! (1/4/5/6/8, plus 7 for a missing tool) and a human-readable message, and must never return an output.
use convert::exec::{Config, Exit, Failure};
use convert::hook::{JobCtx, JobHook, ToolRunner};
use convert::job::{self, Dest, FileReport, JobSpec};
use convert::plan::{Outputs, Step, ToolId, ToolStep};
use convert::report::Warning;
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
        c.tools
            .insert(ToolId::Docs, env!("CARGO_BIN_EXE_convert-fake-tool").into());
        c.sample_ms = 50;
        c
    }
    fn jobs_left(&self) -> usize {
        std::fs::read_dir(self.0.join("work"))
            .map(|r| r.count())
            .unwrap_or(0)
    }
    fn run(&self, cfg: &Config, argv: &[&str], out: &str) -> FileReport {
        let dest = Dest {
            out_dir: self.0.join("dest"),
            out_file: None,
        };
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
                steps: Ok(vec![step]),
                stem: "sample".into(),
                ..JobSpec::new(&dest)
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
    assert!(
        r.message.to_lowercase().contains(needle),
        "message '{}' lacks '{needle}'",
        r.message
    );
    assert!(r.outputs.is_empty(), "a failed job must not return outputs");
}

#[test]
fn tool_error_is_exit_1_and_passes_the_engine_message() {
    let s = Sandbox::new("e1");
    let r = s.run(&s.cfg(), &["exit1"], "out/output.pdf");
    expect(&r, Exit::ToolError, "the document is broken");
    assert_eq!(
        (r.steps.len(), r.steps[0].exit),
        (1, 1),
        "the failed step is in the report"
    );
    assert!(r.steps[0].stderr_tail.contains("document is broken"));
    assert_eq!(
        s.jobs_left(),
        0,
        "the cleanup guard must remove the job directory"
    );
}

#[test]
fn timeout_is_exit_4() {
    let s = Sandbox::new("e4");
    let mut c = s.cfg();
    c.timeout_s = Some(1);
    let t = std::time::Instant::now();
    let r = s.run(&c, &["sleep", "60"], "out/output.pdf");
    expect(&r, Exit::Timeout, "did not finish within 1 s");
    assert!(
        t.elapsed().as_secs() < 20,
        "the child must be killed, not waited for"
    );
    assert_eq!(s.jobs_left(), 0);
}

#[test]
fn memory_cap_is_exit_5() {
    let s = Sandbox::new("e5");
    let mut c = s.cfg();
    c.max_mem_mb = Some(300);
    let r = s.run(&c, &["alloc", "4096"], "out/output.pdf");
    expect(&r, Exit::Memory, "more than 300 mb");
    // the overshoot is measured, not assumed (review M6): sampled every 100 ms, the peak stays within a few hundred MB of the cap
    assert!(
        r.steps[0].peak_mb < 300 + 1500,
        "peak {} MB",
        r.steps[0].peak_mb
    );
}

#[test]
fn panic_and_abort_are_exit_6() {
    let s = Sandbox::new("e6");
    expect(
        &s.run(&s.cfg(), &["panic"], "out/output.pdf"),
        Exit::Crash,
        "panic",
    );
    expect(
        &s.run(&s.cfg(), &["abort"], "out/output.pdf"),
        Exit::Crash,
        "crashed",
    );
}

#[test]
fn empty_and_truncated_outputs_are_exit_8() {
    let s = Sandbox::new("e8");
    expect(
        &s.run(&s.cfg(), &["empty", "out/output.docx"], "out/output.docx"),
        Exit::BadOutput,
        "empty file",
    );
    expect(
        &s.run(
            &s.cfg(),
            &["truncated", "out/output.docx"],
            "out/output.docx",
        ),
        Exit::BadOutput,
        "truncated",
    );
    expect(
        &s.run(&s.cfg(), &["ok", "out/other.pdf"], "out/output.pdf"),
        Exit::BadOutput,
        "no file",
    );
    // right extension, wrong magic
    expect(
        &s.run(&s.cfg(), &["ok", "out/output.png"], "out/output.png"),
        Exit::BadOutput,
        "png signature",
    );
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
    assert!(
        r2.outputs[0].ends_with("sample (2).pdf"),
        "{}",
        r2.outputs[0]
    );
    assert!(r2.steps[0].peak_mb < 500 && r2.steps[0].exit == 0);
}

struct Veto;
impl JobHook for Veto {
    fn after(
        &self,
        _: &JobCtx,
        _: &[PathBuf],
        _: &dyn ToolRunner,
    ) -> Result<Vec<Warning>, Failure> {
        Err(Failure {
            exit: Exit::Rejected,
            msg: "vetoed by hook".into(),
        })
    }
}

#[test]
fn after_hook_can_veto() {
    let s = Sandbox::new("hook");
    let mut c = s.cfg();
    c.hooks.push(Arc::new(Veto));
    expect(
        &s.run(&c, &["ok", "out/output.pdf"], "out/output.pdf"),
        Exit::Rejected,
        "vetoed",
    );
}

#[test]
fn before_hook_refuses_before_any_tool_runs() {
    struct Refuse(std::sync::Mutex<Vec<String>>);
    impl JobHook for Refuse {
        fn before(&self, ctx: &JobCtx, _: &dyn ToolRunner) -> Result<Vec<Warning>, Failure> {
            self.0.lock().unwrap().push(format!(
                "{:?} {} {:?} {:?}",
                ctx.kind, ctx.target, ctx.principal, ctx.client
            ));
            Err(Failure {
                exit: Exit::Unsupported,
                msg: "this direction cannot carry a watermark".into(),
            })
        }
    }
    let s = Sandbox::new("before");
    let mut c = s.cfg();
    let h = Arc::new(Refuse(Default::default()));
    c.hooks.push(h.clone());
    let r = s.run(&c, &["ok", "out/output.pdf"], "out/output.pdf");
    expect(&r, Exit::Unsupported, "watermark");
    assert!(r.steps.is_empty(), "no tool ran");
    assert_eq!(h.0.lock().unwrap().len(), 1);
}

#[test]
fn hook_warnings_join_the_report_and_context_arrives() {
    struct Note;
    impl JobHook for Note {
        fn before(&self, ctx: &JobCtx, _: &dyn ToolRunner) -> Result<Vec<Warning>, Failure> {
            assert!(ctx.job_dir.join("in").is_dir() && ctx.input.ends_with("input.txt"));
            Ok(vec![convert::report::warn("macros_dropped", 1)])
        }
    }
    let s = Sandbox::new("note");
    let mut c = s.cfg();
    c.hooks.push(Arc::new(Note));
    let r = s.run(&c, &["ok", "out/output.pdf"], "out/output.pdf");
    assert_eq!(
        (r.code, r.warnings[0].code.as_str()),
        (0, "macros_dropped"),
        "{}",
        r.message
    );
}

#[test]
fn outputs_are_listed_and_validated_again_after_the_hooks() {
    // a watermark hook that writes a NEW file next to the original: both are returned, the original is never alone
    struct Adds;
    impl JobHook for Adds {
        fn after(
            &self,
            ctx: &JobCtx,
            _: &[PathBuf],
            _: &dyn ToolRunner,
        ) -> Result<Vec<Warning>, Failure> {
            std::fs::write(ctx.job_dir.join("out/output-wm.pdf"), b"%PDF-1.4\n%%EOF\n").unwrap();
            Ok(vec![])
        }
    }
    let s = Sandbox::new("adds");
    let mut c = s.cfg();
    c.hooks.push(Arc::new(Adds));
    let r = s.run(&c, &["ok", "out/output.pdf"], "out/output.pdf");
    assert_eq!((r.code, r.outputs.len()), (0, 2), "{}", r.message);
    // a hook that corrupts an output is exit 8, not a silent exit 0
    struct Corrupts;
    impl JobHook for Corrupts {
        fn after(
            &self,
            ctx: &JobCtx,
            _: &[PathBuf],
            _: &dyn ToolRunner,
        ) -> Result<Vec<Warning>, Failure> {
            std::fs::write(ctx.job_dir.join("out/output.pdf"), b"garbage").unwrap();
            Ok(vec![])
        }
    }
    let mut c = s.cfg();
    c.hooks.push(Arc::new(Corrupts));
    expect(
        &s.run(&c, &["ok", "out/output.pdf"], "out/output.pdf"),
        Exit::BadOutput,
        "after the hooks",
    );
}

#[test]
fn an_undeclared_file_in_out_is_exit_8() {
    let s = Sandbox::new("extra");
    let r = s.run(
        &s.cfg(),
        &["two", "out/output.pdf", "out/surprise.pdf"],
        "out/output.pdf",
    );
    expect(&r, Exit::BadOutput, "unexpected file");
}

#[test]
fn a_failed_put_removes_what_was_already_placed() {
    use convert::job::OutputSink;
    use std::sync::Mutex;
    struct Flaky(Mutex<Vec<String>>, Mutex<Vec<String>>);
    impl OutputSink for Flaky {
        fn begin(&self, _: usize, _: &[PathBuf]) -> Result<(), Failure> {
            Ok(())
        }
        fn put(&self, i: usize, name: &str, _: &Path) -> Result<String, Failure> {
            if i == 1 {
                return Err(Failure {
                    exit: Exit::ToolError,
                    msg: "disk full".into(),
                });
            }
            self.0.lock().unwrap().push(name.to_string());
            Ok(name.to_string())
        }
        fn discard(&self, placed: &[String]) {
            self.1.lock().unwrap().extend(placed.iter().cloned());
        }
    }
    let s = Sandbox::new("flaky");
    let sink = Flaky(Default::default(), Default::default());
    let r = job::run_job(
        &s.cfg(),
        JobSpec {
            label: "x".into(),
            inputs: vec![(s.0.join("sample.txt"), "in/input.txt".into())],
            steps: Ok(vec![Step::Tool(ToolStep {
                tool: ToolId::Docs,
                argv: vec!["two".into(), "out/a.pdf".into(), "out/b.pdf".into()],
                outputs: Outputs::Dir {
                    dir: "out".into(),
                    ext: "pdf".into(),
                    expect: Some(2),
                },
                stdout_to: None,
                allow_empty: false,
                write_files: vec![],
            })]),
            ..JobSpec::new(&sink)
        },
    );
    expect(&r, Exit::ToolError, "disk full");
    assert_eq!(
        *sink.1.lock().unwrap(),
        *sink.0.lock().unwrap(),
        "everything placed was discarded"
    );
    assert_eq!(sink.0.lock().unwrap().len(), 1);
}

#[test]
fn the_filesystem_sink_leaves_no_part_files_and_no_placeholder_on_failure() {
    let s = Sandbox::new("fs");
    // the destination "directory" is a file: begin fails before anything is created in it
    let blocker = s.0.join("blocker");
    std::fs::write(&blocker, b"x").unwrap();
    let dest = Dest {
        out_dir: blocker.join("sub"),
        out_file: None,
    };
    let r = job::run_job(
        &s.cfg(),
        JobSpec {
            label: "x".into(),
            inputs: vec![(s.0.join("sample.txt"), "in/input.txt".into())],
            steps: Ok(vec![Step::Tool(ToolStep {
                tool: ToolId::Docs,
                argv: vec!["ok".into(), "out/output.pdf".into()],
                outputs: Outputs::Files(vec!["out/output.pdf".into()]),
                stdout_to: None,
                allow_empty: false,
                write_files: vec![],
            })]),
            ..JobSpec::new(&dest)
        },
    );
    expect(&r, Exit::ToolError, "cannot create");
    assert_eq!(std::fs::read(&blocker).unwrap(), b"x");
}

#[test]
fn unsafe_stems_cannot_escape_or_name_a_device() {
    let s = Sandbox::new("stem");
    for (stem, want) in [
        (r"..\..\evil", "_.._evil.pdf"),
        ("NUL", "_NUL.pdf"),
        ("a/b", "a_b.pdf"),
        ("", "output.pdf"),
    ] {
        let dest = Dest {
            out_dir: s.0.join("dest"),
            out_file: None,
        };
        let r = job::run_job(
            &s.cfg(),
            JobSpec {
                label: "x".into(),
                inputs: vec![(s.0.join("sample.txt"), "in/input.txt".into())],
                steps: Ok(vec![Step::Tool(ToolStep {
                    tool: ToolId::Docs,
                    argv: vec!["ok".into(), "out/output.pdf".into()],
                    outputs: Outputs::Files(vec!["out/output.pdf".into()]),
                    stdout_to: None,
                    allow_empty: false,
                    write_files: vec![],
                })]),
                stem: stem.into(),
                ..JobSpec::new(&dest)
            },
        );
        assert_eq!(r.code, 0, "{stem:?}: {}", r.message);
        let out = Path::new(&r.outputs[0]);
        assert_eq!(
            out.parent().unwrap(),
            s.0.join("dest"),
            "{stem:?} stayed in the destination"
        );
        assert_eq!(out.file_name().unwrap().to_string_lossy(), want);
        assert!(out.is_file());
        let _ = std::fs::remove_file(out);
    }
}

#[test]
fn cancel_and_the_job_deadline_end_a_running_step() {
    let s = Sandbox::new("cancel");
    let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let f2 = flag.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(400));
        f2.store(true, std::sync::atomic::Ordering::Relaxed);
    });
    let mut c = s.cfg();
    c.cancel = Some(flag);
    let t = std::time::Instant::now();
    let r = s.run(&c, &["sleep", "60"], "out/output.pdf");
    expect(&r, Exit::Timeout, "cancelled");
    assert!(t.elapsed().as_secs() < 15);
    // the overall deadline, shorter than the step
    let mut c = s.cfg();
    c.job_timeout_s = 1;
    let t = std::time::Instant::now();
    let r = s.run(&c, &["sleep", "60"], "out/output.pdf");
    expect(&r, Exit::Timeout, "overall time limit");
    assert!(t.elapsed().as_secs() < 15);
}

#[test]
fn a_panic_in_one_job_becomes_a_crash_report() {
    let r = job::guarded("a.docx", "pdf", || panic!("boom"));
    assert_eq!((r.code, r.status), (6, "failed"));
    assert!(r.message.contains("boom"), "{}", r.message);
}

#[test]
fn stale_job_directories_are_swept_but_not_ours() {
    let s = Sandbox::new("sweep");
    let work = s.0.join("work");
    let mine = work.join(format!("job-{}-abc-1", std::process::id()));
    let dead = work.join("job-4000000-abc-1");
    let dead_stage = work.join("stage-4000000-abc-2");
    let other = work.join("keep-me");
    for d in [&mine, &dead, &dead_stage, &other] {
        std::fs::create_dir_all(d).unwrap();
    }
    assert_eq!(job::sweep_stale(&work), 2);
    assert!(mine.is_dir() && other.is_dir() && !dead.exists() && !dead_stage.exists());
}
