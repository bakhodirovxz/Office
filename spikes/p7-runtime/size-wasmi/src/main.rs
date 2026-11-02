// Size probe: the baseline plus loading and calling one export through wasmi.
fn main() {
    let b = std::fs::read(std::env::args().nth(1).unwrap()).unwrap();
    let engine = wasmi::Engine::default();
    let module = wasmi::Module::new(&engine, &b[..]).unwrap();
    let mut store = wasmi::Store::new(&engine, ());
    let inst = wasmi::Linker::<()>::new(&engine).instantiate_and_start(&mut store, &module).unwrap();
    let f = inst.get_typed_func::<u32, u32>(&store, "p7_alloc").unwrap();
    println!("{}", serde_json::json!({"len": b.len(), "p": f.call(&mut store, 16).unwrap()}));
}
