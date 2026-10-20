//! Micro-benchmark: `cargo test --release -p uz-translit -- --ignored throughput --nocapture`
use std::time::Instant;
use uz_translit::{Direction, Options, convert};

const SAMPLE_LAT: &str = "Oʻzbekiston Respublikasi Mehnat kodeksi: xodimning ish haqi, taʼtil va mehnat shartnomasi. \
1-sentabrdan boshlab operatsiya, funksiyasi, kompyuter va obyekt boʻyicha buyruq (PF-1234-son) https://lex.uz/docs/5013007. \
Isʼhoq Rashidov MChJ direktori, Toshkent shahri, Chilonzor tumani; tel: +998 90 123-45-67, info@lex.uz. XXI asr, 3x4. ";

fn mixed(bytes: usize) -> String {
    let cy = convert(SAMPLE_LAT, Direction::ToCyrillic, &Options::default());
    let mut s = String::with_capacity(bytes + 1000);
    while s.len() < bytes {
        s.push_str(SAMPLE_LAT);
        s.push_str(&cy);
    }
    s
}

fn mbps(text: &str, dir: Direction) -> f64 {
    let o = Options::default();
    let mut best = f64::MAX;
    for _ in 0..5 {
        let t = Instant::now();
        let out = convert(text, dir, &o);
        best = best.min(t.elapsed().as_secs_f64());
        assert!(!out.is_empty());
    }
    text.len() as f64 / best / 1e6
}

#[test]
#[ignore]
fn throughput() {
    let text = mixed(1 << 20);
    let lat: String = text.clone();
    let (a, b) = (
        mbps(&lat, Direction::ToCyrillic),
        mbps(&lat, Direction::ToLatin),
    );
    let kb300 = mixed(300_000);
    let t = Instant::now();
    let _ = convert(&kb300, Direction::ToCyrillic, &Options::default());
    let ms = t.elapsed().as_secs_f64() * 1e3;
    println!(
        "throughput 1 MB mixed ({} bytes): ->C {a:.1} MB/s, ->L {b:.1} MB/s; 300 KB ->C in {ms:.1} ms",
        lat.len()
    );
    assert!(a > 3.0 && b > 3.0, "300 KB must stay far under 100 ms");
}
