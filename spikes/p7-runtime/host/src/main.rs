//! Spike bench: native uz-translit vs the same code as a wasm plugin under wasmtime and wasmi, with the binary and the JSON
//! batch formats. Prints one line per measurement (median of ROUNDS). Usage: p7-host <plugin.wasm> [chars]
#[path = "../../wire.rs"]
mod wire;
use std::borrow::Cow;
use std::time::Instant;
use uz_translit::{Direction, LatinAlphabet, Options, Run, convert, convert_runs};
use wire::*;

const ROUNDS: usize = 7;
const SAMPLE: &str = "Oʻzbekiston Respublikasi Mehnat kodeksi: xodimning ish haqi, taʼtil va mehnat shartnomasi. \
1-sentabrdan boshlab operatsiya, funksiyasi, kompyuter va obyekt boʻyicha buyruq (PF-1234-son) https://lex.uz/docs/5013007. \
Isʼhoq Rashidov MChJ direktori, Toshkent shahri, Chilonzor tumani; tel: +998 90 123-45-67, info@lex.uz. XXI asr, 3x4. ";

type Paras = Vec<Vec<(String, bool)>>;
type Outs = Vec<Vec<Option<String>>>;

fn opts() -> Options {
    Options { latin_alphabet: LatinAlphabet::Y1995, ..Options::default() }
}

/// Paragraphs of about 110 chars, cut into runs of 1 to 5 words; every 7th run is a "keep" run (a skipped language).
fn corpus(chars: usize) -> Paras {
    let words: Vec<&str> = SAMPLE.split(' ').filter(|w| !w.is_empty()).collect();
    let (mut paras, mut n, mut wi, mut ri) = (Vec::new(), 0usize, 0usize, 0usize);
    while n < chars {
        let mut p = Vec::new();
        let mut plen = 0;
        while plen < 110 {
            let k = 1 + (ri % 5);
            let mut t = String::new();
            for _ in 0..k {
                t.push_str(words[wi % words.len()]);
                t.push(' ');
                wi += 1;
            }
            plen += t.chars().count();
            p.push((t, ri % 7 != 6));
            ri += 1;
        }
        n += plen;
        paras.push(p);
    }
    paras
}

fn enc_bin(paras: &Paras) -> Vec<u8> {
    let mut o = vec![0u8];
    put_u32(&mut o, paras.len() as u32);
    for p in paras {
        put_u32(&mut o, p.len() as u32);
        for (t, c) in p {
            o.push(*c as u8);
            put_str(&mut o, t);
        }
    }
    o
}

fn dec_bin(b: &[u8]) -> Outs {
    let mut r = Rd { b, i: 0 };
    let n = r.u32();
    (0..n)
        .map(|_| {
            let k = r.u32();
            (0..k).map(|_| (r.u8() == 1).then(|| r.str().to_string())).collect()
        })
        .collect()
}

fn enc_json(paras: &Paras) -> Vec<u8> {
    let p: Vec<serde_json::Value> = paras
        .iter()
        .map(|p| p.iter().map(|(t, c)| serde_json::json!({"t": t, "c": c})).collect())
        .collect();
    serde_json::to_vec(&serde_json::json!({"dir": "cyrl", "paras": p})).unwrap()
}

fn dec_json(b: &[u8]) -> Outs {
    serde_json::from_slice(b).unwrap()
}

fn native(paras: &Paras) -> Outs {
    let o = opts();
    paras
        .iter()
        .map(|p| {
            let runs: Vec<Run> = p.iter().map(|(t, c)| Run { text: t, convert: *c }).collect();
            convert_runs(&runs, Direction::ToCyrillic, &o)
                .into_iter()
                .map(|c| match c {
                    Cow::Borrowed(_) => None,
                    Cow::Owned(s) => Some(s),
                })
                .collect()
        })
        .collect()
}

fn med(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn time<T>(mut f: impl FnMut() -> T) -> (f64, T) {
    let mut ts = Vec::new();
    let mut last = None;
    for _ in 0..ROUNDS {
        let t = Instant::now();
        last = Some(f());
        ts.push(t.elapsed().as_secs_f64() * 1e3);
    }
    (med(ts), last.unwrap())
}

trait Guest {
    fn call(&mut self, export: &str, input: &[u8]) -> Vec<u8>;
}

struct Wt {
    store: wasmtime::Store<()>,
    inst: wasmtime::Instance,
}
impl Guest for Wt {
    fn call(&mut self, export: &str, input: &[u8]) -> Vec<u8> {
        let s = &mut self.store;
        s.set_epoch_deadline(1_000_000);
        let mem = self.inst.get_memory(&mut *s, "memory").unwrap();
        let alloc = self.inst.get_typed_func::<u32, u32>(&mut *s, "p7_alloc").unwrap();
        let free = self.inst.get_typed_func::<(u32, u32), ()>(&mut *s, "p7_free").unwrap();
        let f = self.inst.get_typed_func::<(u32, u32), u64>(&mut *s, export).unwrap();
        let p = alloc.call(&mut *s, input.len() as u32).unwrap();
        mem.write(&mut *s, p as usize, input).unwrap();
        let r = f.call(&mut *s, (p, input.len() as u32)).unwrap();
        let (op, on) = ((r >> 32) as u32, r as u32);
        let mut out = vec![0u8; on as usize];
        mem.read(&*s, op as usize, &mut out).unwrap();
        free.call(&mut *s, (op, on)).unwrap();
        out
    }
}

struct Wi {
    store: wasmi::Store<()>,
    inst: wasmi::Instance,
}
impl Guest for Wi {
    fn call(&mut self, export: &str, input: &[u8]) -> Vec<u8> {
        let s = &mut self.store;
        let mem = self.inst.get_memory(&*s, "memory").unwrap();
        let alloc = self.inst.get_typed_func::<u32, u32>(&*s, "p7_alloc").unwrap();
        let free = self.inst.get_typed_func::<(u32, u32), ()>(&*s, "p7_free").unwrap();
        let f = self.inst.get_typed_func::<(u32, u32), u64>(&*s, export).unwrap();
        let p = alloc.call(&mut *s, input.len() as u32).unwrap();
        mem.write(&mut *s, p as usize, input).unwrap();
        let r = f.call(&mut *s, (p, input.len() as u32)).unwrap();
        let (op, on) = ((r >> 32) as u32, r as u32);
        let mut out = vec![0u8; on as usize];
        mem.read(&*s, op as usize, &mut out).unwrap();
        free.call(&mut *s, (op, on)).unwrap();
        out
    }
}

fn bench(name: &str, g: &mut dyn Guest, paras: &Paras, want: &Outs) {
    let (t, out) = time(|| dec_bin(&g.call("p7_bin", &enc_bin(paras))));
    assert!(&out == want, "{name} bin output differs from native");
    println!("{name:<9} bin  round trip (encode+copy+convert+copy+decode) {t:8.1} ms  identical=true");
    let (t, out) = time(|| dec_json(&g.call("p7_json", &enc_json(paras))));
    assert!(&out == want, "{name} json output differs from native");
    println!("{name:<9} json round trip                                  {t:8.1} ms  identical=true");
}

fn main() -> wasmtime::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let wasm = std::fs::read(&args[1])?;
    let chars: usize = args.get(2).map_or(330_000, |s| s.parse().unwrap());
    let paras = corpus(chars);
    let runs: usize = paras.iter().map(|p| p.len()).sum();
    let bytes: usize = paras.iter().flatten().map(|(t, _)| t.len()).sum();
    println!("corpus: {} paragraphs, {runs} runs, {bytes} bytes UTF-8, plugin {} bytes", paras.len(), wasm.len());

    let (t, want) = time(|| native(&paras));
    println!("native    convert_runs per paragraph                    {t:8.1} ms");
    // For the browser bench (web/bench.html): the same input and the native answer in the plugin's output format.
    if let Some(dir) = args.get(3) {
        std::fs::write(format!("{dir}/input.bin"), enc_bin(&paras))?;
        let mut o = Vec::new();
        put_u32(&mut o, want.len() as u32);
        for p in &want {
            put_u32(&mut o, p.len() as u32);
            for r in p {
                match r {
                    None => o.push(0),
                    Some(s) => {
                        o.push(1);
                        put_str(&mut o, s);
                    }
                }
            }
        }
        std::fs::write(format!("{dir}/native.bin"), o)?;
    }
    let whole: String = paras.iter().flatten().map(|(t, _)| t.as_str()).collect();
    let (t, _) = time(|| convert(&whole, Direction::ToCyrillic, &opts()));
    println!("native    convert(whole text)                           {t:8.1} ms");
    let (t, _) = time(|| enc_json(&paras));
    println!("host      json encode only                              {t:8.1} ms");
    let (t, _) = time(|| enc_bin(&paras));
    println!("host      bin encode only                               {t:8.1} ms");

    // wasmtime: cranelift, epoch interruption on (what a product host would run with).
    let t0 = Instant::now();
    let mut cfg = wasmtime::Config::new();
    cfg.epoch_interruption(true);
    let engine = wasmtime::Engine::new(&cfg)?;
    let t_engine = t0.elapsed().as_secs_f64() * 1e3;
    let t0 = Instant::now();
    let module = wasmtime::Module::new(&engine, &wasm)?;
    let t_compile = t0.elapsed().as_secs_f64() * 1e3;
    let t0 = Instant::now();
    let mut store = wasmtime::Store::new(&engine, ());
    store.set_epoch_deadline(1_000_000);
    let inst = wasmtime::Linker::new(&engine).instantiate(&mut store, &module)?;
    let t_inst = t0.elapsed().as_secs_f64() * 1e3;
    println!(
        "wasmtime  engine {t_engine:.1} ms, compile {t_compile:.1} ms, instantiate {t_inst:.2} ms; imports {}",
        module.imports().len()
    );
    bench("wasmtime", &mut Wt { store, inst }, &paras, &want);

    // wasmi: interpreter, fuel metering on (its way of stopping a runaway plugin).
    let t0 = Instant::now();
    let mut wcfg = wasmi::Config::default();
    wcfg.consume_fuel(true);
    let wengine = wasmi::Engine::new(&wcfg);
    let wmodule = wasmi::Module::new(&wengine, &wasm[..]).map_err(wasmtime::Error::msg)?;
    let t_compile = t0.elapsed().as_secs_f64() * 1e3;
    let t0 = Instant::now();
    let mut wstore = wasmi::Store::new(&wengine, ());
    wstore.set_fuel(u64::MAX / 2).map_err(wasmtime::Error::msg)?;
    let winst = wasmi::Linker::<()>::new(&wengine)
        .instantiate_and_start(&mut wstore, &wmodule)
        .map_err(wasmtime::Error::msg)?;
    let t_inst = t0.elapsed().as_secs_f64() * 1e3;
    println!("wasmi     engine+translate {t_compile:.1} ms, instantiate {t_inst:.2} ms");
    bench("wasmi", &mut Wi { store: wstore, inst: winst }, &paras, &want);

    // Sandbox: a spinning plugin stopped by the epoch deadline; a memory hog capped by the store limits.
    let mut store = wasmtime::Store::new(&engine, wasmtime::StoreLimitsBuilder::new().memory_size(64 << 20).build());
    store.limiter(|l| l);
    store.set_epoch_deadline(1);
    let inst = wasmtime::Linker::new(&engine).instantiate(&mut store, &module)?;
    let spin = inst.get_typed_func::<(), u32>(&mut store, "p7_spin")?;
    let e2 = engine.clone();
    let ticker = std::thread::spawn(move || {
        for _ in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            e2.increment_epoch();
        }
    });
    let t0 = Instant::now();
    let r = spin.call(&mut store, ());
    let err = r.err().map(|e| e.to_string().lines().next().unwrap_or("").to_string());
    println!("wasmtime  spin: stopped={:?} after {:.0} ms", err, t0.elapsed().as_secs_f64() * 1e3);
    ticker.join().unwrap();
    store.set_epoch_deadline(1_000_000);
    let hog = inst.get_typed_func::<(), u32>(&mut store, "p7_hog")?;
    println!("wasmtime  hog: got {} pages (64 KiB each) under a 64 MiB limit", hog.call(&mut store, ())?);
    Ok(())
}
