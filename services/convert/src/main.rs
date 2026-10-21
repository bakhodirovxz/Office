//! `convert`: the command line (PLAN phase 3, section 2.2). Hand-rolled argument parsing, no clap: the grammar is small.
use convert::exec::{Config, Exit, run_tool};
use convert::formats;
use convert::job::{self, Dest, FileReport};
use convert::plan::{self, Options, PageSpec, PdfEngine, SheetSel, SplitBy, Target, ToolId, Translit, TranslitDir};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
usage:
  convert <inputs...> --to <fmt> [-o FILE | --out-dir DIR] [options]
      fmt: docx odt rtf html md txt pdf png xlsx csv pptx
      --translit cyrl|latn|auto   --alphabet 1995|2026|auto (default 1995)   --apostrophe official|ascii
      --sheet NAME|active|all     (csv; default active)
      --pages 1,3-5               (png output; pdf to txt takes one page)
      --dpi N                     (png; 72-600, default 150)
      --pdf-engine sidecar|native (pdf to docx; default native until the sidecar exists)
      --timeout SECS  --max-mem MB  -j N  --json  --strict  --verify  --keep-work  --work-dir DIR  --max-input-mb N
  convert pdf combine <a.pdf> <b.pdf>... -o out.pdf
  convert pdf split <in.pdf> (--every N | --before 3,7) --out-dir DIR
  convert pdf extract <in.pdf> --pages 1,3-5 -o out.pdf
  convert formats [--json]
  convert check
exit codes: 0 ok, 1 tool error, 2 usage, 3 unsupported, 4 timeout, 5 memory cap, 6 tool crashed, 7 tool missing,
            8 output failed validation, 9 input rejected, 10 warnings under --strict";

const VALUE_FLAGS: &[&str] = &[
    "--to", "-o", "--out-dir", "--translit", "--alphabet", "--apostrophe", "--sheet", "--pages", "--dpi", "--pdf-engine", "--timeout",
    "--max-mem", "-j", "--every", "--before", "--work-dir", "--max-input-mb",
];
const BOOL_FLAGS: &[&str] = &["--json", "--strict", "--keep-work", "--verify"];

struct Args {
    pos: Vec<String>,
    val: HashMap<String, String>,
    flag: Vec<String>,
}

fn parse(argv: Vec<String>) -> Result<Args, String> {
    let (mut pos, mut val, mut flag) = (vec![], HashMap::new(), vec![]);
    let mut it = argv.into_iter();
    while let Some(a) = it.next() {
        if VALUE_FLAGS.contains(&a.as_str()) {
            let v = it.next().ok_or(format!("{a} needs a value"))?;
            val.insert(a, v);
        } else if BOOL_FLAGS.contains(&a.as_str()) {
            flag.push(a);
        } else if a.starts_with('-') && a.len() > 1 {
            return Err(format!("unknown option {a}"));
        } else {
            pos.push(a);
        }
    }
    Ok(Args { pos, val, flag })
}

impl Args {
    fn num<T: std::str::FromStr>(&self, k: &str) -> Result<Option<T>, String> {
        self.val.get(k).map(|v| v.parse::<T>().map_err(|_| format!("{k} needs a number, got '{v}'"))).transpose()
    }
}

fn usage_err(m: impl std::fmt::Display) -> ExitCode {
    eprintln!("convert: {m}\n\n{USAGE}");
    ExitCode::from(Exit::Usage.code())
}

fn config(a: &Args) -> Result<Config, String> {
    let mut c = Config::from_env();
    c.timeout_s = a.num("--timeout")?;
    c.max_mem_mb = a.num("--max-mem")?;
    if let Some(m) = a.num("--max-input-mb")? {
        c.max_input_mb = m;
    }
    if let Some(d) = a.val.get("--work-dir") {
        c.work_dir = d.into();
    }
    c.keep_work = a.flag.iter().any(|f| f == "--keep-work");
    c.verify = a.flag.iter().any(|f| f == "--verify");
    Ok(c)
}

fn options(a: &Args) -> Result<Options, String> {
    let mut o = Options::default();
    if let Some(t) = a.val.get("--translit") {
        let dir = match t.as_str() {
            "cyrl" => TranslitDir::Cyrl,
            "latn" => TranslitDir::Latn,
            "auto" => TranslitDir::Auto,
            _ => return Err(format!("--translit takes cyrl, latn or auto, got '{t}'")),
        };
        let alphabet = match a.val.get("--alphabet").map(String::as_str).unwrap_or("1995") {
            "1995" => "1995",
            "2026" => "2026",
            "auto" => "auto",
            x => return Err(format!("--alphabet takes 1995, 2026 or auto, got '{x}'")),
        };
        let apostrophe = match a.val.get("--apostrophe").map(String::as_str).unwrap_or("official") {
            "official" => "official",
            "ascii" => "ascii",
            x => return Err(format!("--apostrophe takes official or ascii, got '{x}'")),
        };
        o.translit = Some(Translit { dir, alphabet, apostrophe });
    } else if a.val.contains_key("--alphabet") || a.val.contains_key("--apostrophe") {
        return Err("--alphabet and --apostrophe need --translit".into());
    }
    o.sheet = match a.val.get("--sheet").map(String::as_str) {
        None | Some("active") => SheetSel::Active,
        Some("all") => SheetSel::All,
        Some(n) => SheetSel::Name(n.into()),
    };
    if let Some(p) = a.val.get("--pages") {
        o.pages = Some(PageSpec::parse(p)?);
    }
    if let Some(d) = a.num::<u32>("--dpi")? {
        if !(72..=600).contains(&d) {
            return Err("--dpi must be between 72 and 600".into());
        }
        o.dpi = d;
    }
    o.pdf_engine = match a.val.get("--pdf-engine").map(String::as_str) {
        None => PdfEngine::default(),
        Some("native") => PdfEngine::Native,
        Some("sidecar") => PdfEngine::Sidecar,
        Some(x) => return Err(format!("--pdf-engine takes sidecar or native, got '{x}'")),
    };
    Ok(o)
}

fn finish(mut reports: Vec<FileReport>, a: &Args) -> ExitCode {
    let strict = a.flag.iter().any(|f| f == "--strict");
    let json = a.flag.iter().any(|f| f == "--json");
    for r in &mut reports {
        if strict {
            r.apply_strict();
        }
        for w in &r.warnings {
            eprintln!("{}: warning [{}]: {}", r.input, w.code, w.text);
        }
        if r.code != 0 {
            eprintln!("{}: {} (exit {})", r.input, r.message, r.code);
        } else if !json {
            for o in &r.outputs {
                println!("{o}");
            }
        }
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&reports).expect("report serialises"));
    }
    ExitCode::from(job::batch_exit(&reports))
}

fn dest(a: &Args, many: bool) -> Result<Dest, String> {
    if a.val.contains_key("-o") && a.val.contains_key("--out-dir") {
        return Err("use either -o or --out-dir".into());
    }
    if a.val.contains_key("-o") && many {
        return Err("-o names one file; use --out-dir with several inputs".into());
    }
    Ok(Dest { out_dir: a.val.get("--out-dir").map(PathBuf::from).unwrap_or_else(|| ".".into()), out_file: a.val.get("-o").map(PathBuf::from) })
}

fn cmd_convert(a: Args) -> ExitCode {
    let Some(to) = a.val.get("--to") else { return usage_err("--to is required") };
    let Some(target) = Target::parse(to) else { return usage_err(format!("unknown target '{to}'")) };
    if a.pos.is_empty() {
        return usage_err("no input files");
    }
    let (cfg, opts, d) = match (config(&a), options(&a), dest(&a, a.pos.len() > 1)) {
        (Ok(c), Ok(o), Ok(d)) => (c, o, d),
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return usage_err(e),
    };
    let j = match a.num::<usize>("-j") {
        Ok(j) => j.unwrap_or(1),
        Err(e) => return usage_err(e),
    };
    let inputs: Vec<PathBuf> = a.pos.iter().map(PathBuf::from).collect();
    let reports = job::par_map(&inputs, j, |i| job::convert_file(&cfg, i, target, &opts, &d));
    finish(reports, &a)
}

fn cmd_pdf(mut a: Args) -> ExitCode {
    if a.pos.is_empty() {
        return usage_err("pdf needs combine, split or extract");
    }
    let op = a.pos.remove(0);
    let cfg = match config(&a) {
        Ok(c) => c,
        Err(e) => return usage_err(e),
    };
    let d = match dest(&a, false) {
        Ok(d) => d,
        Err(e) => return usage_err(e),
    };
    let inputs: Vec<PathBuf> = a.pos.iter().map(PathBuf::from).collect();
    let steps = match op.as_str() {
        "combine" if inputs.len() >= 2 => plan::pdf_combine(inputs.len()),
        "combine" => return usage_err("combine needs at least two PDFs"),
        "extract" | "split" if inputs.len() != 1 => return usage_err(format!("{op} takes exactly one PDF")),
        "extract" => match a.val.get("--pages").map(|p| PageSpec::parse(p)) {
            Some(Ok(p)) => plan::pdf_extract(&p),
            Some(Err(e)) => return usage_err(e),
            None => return usage_err("extract needs --pages"),
        },
        "split" => {
            let by = match (a.num::<u32>("--every"), a.val.get("--before")) {
                (Ok(Some(n)), None) if n >= 1 => SplitBy::Every(n),
                (Ok(None), Some(b)) => match b.split(',').map(|x| x.parse::<u32>().ok().filter(|n| *n >= 1)).collect::<Option<Vec<_>>>() {
                    Some(v) if !v.is_empty() => SplitBy::Before(v),
                    _ => return usage_err("--before takes page numbers like 3,7"),
                },
                _ => return usage_err("split needs exactly one of --every N (N >= 1) or --before 3,7"),
            };
            if a.val.contains_key("-o") {
                return usage_err("split writes several files; use --out-dir");
            }
            plan::pdf_split(&by)
        }
        _ => return usage_err(format!("unknown pdf operation '{op}'")),
    };
    let r = job::pdf_op(&cfg, &op, &inputs, steps, &d);
    finish(vec![r], &a)
}

fn cmd_check(a: Args) -> ExitCode {
    let cfg = match config(&a) {
        Ok(c) => c,
        Err(e) => return usage_err(e),
    };
    let dir = std::env::temp_dir();
    let mut missing = false;
    for t in ToolId::ALL {
        let ver = if t == ToolId::Sheets { "version" } else { "--version" };
        match cfg.resolve(t) {
            Err(f) => {
                missing = true;
                println!("{:<11} MISSING  {}", t.name(), f.msg);
            }
            Ok(p) => match run_tool(&cfg, t, &[ver.to_string()], &dir, None, 0.0, 0, &mut vec![]) {
                Ok(r) => println!("{:<11} ok       {}  {}", t.name(), p.display(), String::from_utf8_lossy(&r.stdout).lines().next().unwrap_or("").trim()),
                Err(f) => {
                    missing = true;
                    println!("{:<11} BROKEN   {}  {}", t.name(), p.display(), f.msg);
                }
            },
        }
    }
    println!("sidecars    not configured (pdf2docx, soffice: later work packages)");
    ExitCode::from(if missing { Exit::Missing.code() } else { 0 })
}

fn main() -> ExitCode {
    let mut argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() || argv.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::from(if argv.is_empty() { Exit::Usage.code() } else { 0 });
    }
    let sub = argv[0].clone();
    let rest = if matches!(sub.as_str(), "formats" | "check" | "pdf") { argv.drain(1..).collect() } else { std::mem::take(&mut argv) };
    let a = match parse(rest) {
        Ok(a) => a,
        Err(e) => return usage_err(e),
    };
    match sub.as_str() {
        "formats" => {
            if a.flag.iter().any(|f| f == "--json") {
                println!("{}", serde_json::to_string_pretty(&formats::formats_json()).expect("json"));
            } else {
                print!("{}", formats::formats_text());
            }
            ExitCode::SUCCESS
        }
        "check" => cmd_check(a),
        "pdf" => cmd_pdf(a),
        _ => cmd_convert(a),
    }
}
