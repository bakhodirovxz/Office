//! Test-only stand-in for a domain CLI (PLAN WP2 step 5). The first argument picks the fault; the integration tests in
//! `tests/fault.rs` run the executor against it.
//!   sleep SECS | exit1 | panic | abort | alloc MB | empty PATH | truncated PATH | ok PATH [TEXT] | warn PATH
//!   two A B (writes both) | translit PATH JSON (prints JSON on stdout) | silent (writes nothing, exit 0)
//! and the shapes the planner really emits, so `convert_file` can run end to end without a domain CLI:
//!   convert IN OUT | run ... (--out/--save OUT, or a file.exportPdf path) | info FILE (fails when the file holds BADINFO)
//!   text PDF (prints the file after its first line, with the `<<` and `>>` marks removed: the "text layer")
//!   run --script S (the native pdf to docx: writes a docx of that text with every `<<...>>` span left out: the "lost column")
fn stub(path: &str, extra: &str) {
    let body = if path.ends_with(".pdf") {
        format!("%PDF-1.4\n1 0 obj<<>>endobj\n{extra}\ntrailer<<>>\n%%EOF\n")
    } else {
        format!("stub {extra}\n")
    };
    std::fs::write(path, body).unwrap();
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| a.get(i).map(String::as_str).unwrap_or("");
    match arg(0) {
        "sleep" => std::thread::sleep(std::time::Duration::from_secs(arg(1).parse().unwrap_or(30))),
        "exit1" => {
            eprintln!("fake-tool: the document is broken");
            std::process::exit(1);
        }
        "panic" => panic!("fake-tool: deliberate panic"),
        "abort" => std::process::abort(),
        "alloc" => {
            // touch every page so the working set really grows; the watchdog kills us long before 4 GB when the cap is low
            let mb: usize = arg(1).parse().unwrap_or(4096);
            let mut hold: Vec<Vec<u8>> = vec![];
            for _ in 0..mb / 64 {
                hold.push(vec![1u8; 64 << 20]);
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
            std::hint::black_box(&hold);
        }
        "empty" => std::fs::write(arg(1), b"").unwrap(),
        // a zip header and nothing else: right magic, unreadable directory
        "truncated" => {
            std::fs::write(arg(1), b"PK\x03\x04\x14\x00\x00\x00\x08\x00partial").unwrap()
        }
        "ok" | "warn" => {
            if arg(0) == "warn" {
                eprintln!("Warning: fake-tool dropped the charts");
            }
            stub(arg(1), arg(2));
        }
        "translit" => {
            println!("{}", arg(2));
            stub(arg(1), "");
        }
        "silent" => {}
        // two files in one step: `two A B` (both declared by a test step as A, B stays undeclared unless the test declares it)
        "two" => {
            stub(arg(1), "");
            stub(arg(2), "");
        }
        // the locale probe of `sheets_honours_locale`: a tool that reads `01.03.2019` in `ru` answers with it, one that does not says `1/3/2019`;
        // a marker file `honour-locale` in the work directory (the probe runs in a directory under it) plays the tool's version
        "convert"
            if arg(2) == "o.csv"
                && std::fs::read_to_string(arg(1)).is_ok_and(|t| t.contains("01.03.2019")) =>
        {
            let honours = std::env::var("GRIDCRAFT_NUM_LOCALE").is_ok_and(|l| l == "ru")
                && std::path::Path::new("../honour-locale").exists();
            let date = if honours { "01.03.2019" } else { "1/3/2019" };
            std::fs::write("o.csv", format!("a;b\n3,5;{date}\n")).unwrap();
        }
        // prints the locale the tool was given: `env PATH`
        "env" => stub(
            arg(1),
            &std::env::var("GRIDCRAFT_NUM_LOCALE").unwrap_or_default(),
        ),
        "convert" => {
            // an input that says SLEEP keeps the tool busy (the containment test kills the parent meanwhile)
            if std::fs::read(arg(1)).is_ok_and(|b| b.starts_with(b"SLEEP")) {
                std::thread::sleep(std::time::Duration::from_secs(120));
            }
            stub(arg(2), "")
        }
        "text" => {
            let t = std::fs::read_to_string(arg(1)).unwrap_or_default();
            print!(
                "{}",
                t.split_once('\n')
                    .map_or("", |x| x.1)
                    .replace(['<', '>'], "")
            );
        }
        "run" if a.iter().any(|x| x == "--script") => {
            let script: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(arg(2)).unwrap()).unwrap();
            let arg_of = |tool: &str, key: &str| {
                script
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|s| s["tool"] == tool)
                    .and_then(|s| s["args"][key].as_str())
                    .unwrap()
                    .to_string()
            };
            let text = std::fs::read_to_string(arg_of("doc_open", "path")).unwrap_or_default();
            let (mut kept, mut rest) = (String::new(), text.split_once('\n').map_or("", |x| x.1));
            while let Some((before, after)) = rest.split_once("<<") {
                kept += before;
                rest = after.split_once(">>").map_or("", |x| x.1);
            }
            kept += rest;
            let mut w = zip::ZipWriter::new(
                std::fs::File::create(arg_of("doc_export_office", "path")).unwrap(),
            );
            w.start_file(
                "word/document.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            std::io::Write::write_all(
                &mut w,
                format!("<w:document><w:body><w:p><w:r><w:t>{kept}</w:t></w:r></w:p></w:body></w:document>").as_bytes(),
            )
            .unwrap();
            w.finish().unwrap();
        }
        "run" => {
            let at = a
                .iter()
                .position(|x| x == "--out" || x == "--save")
                .map(|i| a[i + 1].clone());
            let pdf = a
                .iter()
                .find_map(|x| x.strip_prefix("file.exportPdf="))
                .and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok())
                .and_then(|v| v["path"].as_str().map(String::from));
            stub(&at.or(pdf).expect("run needs an output"), "");
        }
        "info" => {
            let data = std::fs::read(arg(1)).unwrap_or_else(|_| {
                eprintln!("fake-tool: cannot open {}", arg(1));
                std::process::exit(1)
            });
            if data.windows(7).any(|w| w == b"BADINFO") {
                eprintln!("fake-tool: not a valid document");
                std::process::exit(1);
            }
            // a file that holds XREFWARN reports a damaged cross-reference table, as pdf-cli does for a cut-off PDF
            if data.windows(8).any(|w| w == b"XREFWARN") {
                println!(
                    "{{\"pages\": 1, \"warnings\": [\"Some document structure (bookmarks, comments, fields) could not be read: failed parsing cross reference table\"]}}"
                );
            } else {
                println!("{{\"pages\": 1}}");
            }
        }
        other => {
            eprintln!("fake-tool: unknown mode '{other}'");
            std::process::exit(2);
        }
    }
}
