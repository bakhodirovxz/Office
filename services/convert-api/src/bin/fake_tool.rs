//! Test-only stand-in for a domain CLI. `convert IN OUT` writes a stub output; an input that starts with `SLEEP` keeps the tool
//! busy for two minutes (the cancel test), and one that starts with `FAIL` makes it exit 1 (the UI spec). `--version` and `info` answer like the real ones.
//! The PDF subcommands (`combine`, `extract`, `split`) write stub PDFs; a PDF that contains `PAGES=n` has n pages (default 1).
const PDF: &str = "%PDF-1.4\n1 0 obj<<>>endobj\ntrailer<<>>\n%%EOF\n";

fn pages(path: &str) -> u32 {
    let b = std::fs::read_to_string(path).unwrap_or_default();
    b.split("PAGES=")
        .nth(1)
        .and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|n| n.parse().ok())
        .unwrap_or(1)
}

fn flag<'a>(a: &'a [String], name: &str) -> &'a str {
    a.iter()
        .position(|x| x == name)
        .and_then(|i| a.get(i + 1))
        .map_or("", String::as_str)
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("convert") => {
            if std::fs::read(&a[1]).is_ok_and(|b| b.starts_with(b"SLEEP")) {
                std::thread::sleep(std::time::Duration::from_secs(120));
            }
            if std::fs::read(&a[1]).is_ok_and(|b| b.starts_with(b"FAIL")) {
                eprintln!("fake tool: cannot read this file");
                std::process::exit(1);
            }
            let body = if a[2].ends_with(".pdf") {
                PDF
            } else {
                "stub\n"
            };
            std::fs::write(&a[2], body).unwrap();
        }
        Some("info") => println!("{{\"pages\": {}}}", pages(&a[1])),
        Some("combine") => {
            // ORDER lists the page counts of the inputs in the order they were given: the tests check the order with it
            let order: Vec<String> = a[1..]
                .iter()
                .take_while(|x| !x.starts_with("--"))
                .map(|f| pages(f).to_string())
                .collect();
            std::fs::write(
                flag(&a, "--out"),
                format!(
                    "%PDF-1.4\n%% ORDER={}\n1 0 obj<<>>endobj\ntrailer<<>>\n%%EOF\n",
                    order.join(",")
                ),
            )
            .unwrap();
        }
        Some("extract") => {
            let total = pages(&a[1]);
            let last = flag(&a, "--pages")
                .split([',', '-'])
                .filter_map(|x| x.parse::<u32>().ok())
                .max()
                .unwrap_or(0);
            if last > total {
                eprintln!("fake tool: page {last} is beyond the last page ({total})");
                std::process::exit(1);
            }
            std::fs::write(flag(&a, "--out"), PDF).unwrap();
        }
        Some("split") => {
            let total = pages(&a[1]);
            let parts = match (flag(&a, "--every"), flag(&a, "--before")) {
                (e, _) if !e.is_empty() => total.div_ceil(e.parse().unwrap_or(1)),
                (_, b) => b.split(',').count() as u32 + 1,
            };
            for i in 1..=parts {
                std::fs::write(format!("{}/part-{i}.pdf", flag(&a, "--out-dir")), PDF).unwrap();
            }
        }
        _ => println!("convert-api-fake-tool"),
    }
}
