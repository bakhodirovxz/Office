// Size baseline: std + serde_json, reads a file and prints a JSON value (what every editor exe already links).
fn main() {
    let b = std::fs::read(std::env::args().nth(1).unwrap()).unwrap();
    println!("{}", serde_json::json!({"len": b.len()}));
}
