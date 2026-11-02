// Size probe: the baseline plus loading and calling one export through wasmtime (cranelift, runtime, std).
fn main() -> wasmtime::Result<()> {
    let b = std::fs::read(std::env::args().nth(1).unwrap())?;
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &b)?;
    let mut store = wasmtime::Store::new(&engine, ());
    let inst = wasmtime::Linker::new(&engine).instantiate(&mut store, &module)?;
    let f = inst.get_typed_func::<u32, u32>(&mut store, "p7_alloc")?;
    println!("{}", serde_json::json!({"len": b.len(), "p": f.call(&mut store, 16)?}));
    Ok(())
}
