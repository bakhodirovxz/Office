//! D6 with the real sheets-cli: an Excel CSV in the uz/ru format (`;`, `dd.mm.yyyy`, `3500000,0`) read under `--locale ru` gives 1 March and a
//! number, under en-US 3 January and text. Skipped (with a message) when no sheets-cli is found (CONVERT_SHEETS_CLI or CONVERT_TOOLS_DIR).
use convert::exec::{Config, Exit};
use convert::job::{self, Dest};
use convert::plan::{NumLocale, Options, Target, ToolId};
use std::io::Read;

fn cell(xlsx: &str, r: &str) -> String {
    let mut z = zip::ZipArchive::new(std::fs::File::open(xlsx).unwrap()).unwrap();
    let mut xml = String::new();
    z.by_name("xl/worksheets/sheet1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    let at = xml.find(&format!("<c r=\"{r}\"")).expect("cell");
    xml[at..xml[at..].find("</c>").map_or(xml.len(), |e| at + e)].to_string()
}

fn convert(loc: NumLocale, dir: &std::path::Path, csv: &std::path::Path) -> Option<String> {
    let mut cfg = Config::from_env();
    cfg.work_dir = dir.join("work");
    cfg.resolve(ToolId::Sheets).ok()?;
    let dest = Dest {
        out_dir: dir.join(format!("out-{}", loc.tag())),
        out_file: None,
    };
    let opts = Options {
        locale: loc,
        ..Options::default()
    };
    let r = job::convert_file(&cfg, csv, Target::Xlsx, &opts, &dest);
    assert_eq!(r.code, Exit::Ok.code(), "{}", r.message);
    Some(r.outputs[0].clone())
}

#[test]
fn a_uz_ru_csv_is_read_in_the_job_locale() {
    let dir = std::env::temp_dir().join(format!("convert-csv-locale-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let csv = dir.join("staff.csv");
    std::fs::write(&csv, "Date;Salary\n01.03.2019;3500000,0\n11.09.2019;12,5\n").unwrap();
    let Some(ru) = convert(NumLocale::Ru, &dir, &csv) else {
        eprintln!("skipped: no sheets-cli (set CONVERT_SHEETS_CLI or CONVERT_TOOLS_DIR)");
        return;
    };
    // 43525 = 2019-03-01, 43719 = 2019-09-11 (Excel's serials)
    assert!(
        cell(&ru, "A2").contains("<v>43525</v>"),
        "{}",
        cell(&ru, "A2")
    );
    assert!(
        cell(&ru, "A3").contains("<v>43719</v>"),
        "{}",
        cell(&ru, "A3")
    );
    assert!(
        cell(&ru, "B2").contains("<v>3500000</v>"),
        "{}",
        cell(&ru, "B2")
    );
    assert!(
        cell(&ru, "B3").contains("<v>12.5</v>"),
        "{}",
        cell(&ru, "B3")
    );
    let uz = convert(NumLocale::UzLatn, &dir, &csv).unwrap();
    assert!(cell(&uz, "A2").contains("<v>43525</v>"));
    // en-US reads the same text as 3 January (43468) and the numbers as text: what the old converter did for everybody
    let en = convert(NumLocale::EnUs, &dir, &csv).unwrap();
    assert!(
        cell(&en, "A2").contains("<v>43468</v>"),
        "{}",
        cell(&en, "A2")
    );
    assert!(cell(&en, "B2").contains("t=\"s\""), "{}", cell(&en, "B2"));
    let _ = std::fs::remove_dir_all(&dir);
}
