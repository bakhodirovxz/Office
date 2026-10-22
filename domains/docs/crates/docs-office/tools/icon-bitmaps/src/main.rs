//! icon-bitmaps <svg-dir> <out.bin> <sizes,comma> <fluent_name>...
//! Output: for each icon, `name\0` then, per size s, s*s alpha bytes (row-major). The SVGs are the 20 px regular Fluent icons.
use resvg::{tiny_skia, usvg};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (dir, out) = (&a[1], &a[2]);
    let sizes: Vec<u32> = a[3].split(',').map(|s| s.parse().unwrap()).collect();
    let mut bin = Vec::new();
    for name in &a[4..] {
        let svg = std::fs::read_to_string(format!("{dir}/{name}_20_regular.svg")).unwrap();
        let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).unwrap();
        bin.extend_from_slice(name.as_bytes());
        bin.push(0);
        for &s in &sizes {
            let mut pm = tiny_skia::Pixmap::new(s, s).unwrap();
            let k = s as f32 / 20.0;
            resvg::render(&tree, tiny_skia::Transform::from_scale(k, k), &mut pm.as_mut());
            bin.extend(pm.data().chunks(4).map(|p| p[3]));
        }
    }
    std::fs::write(out, bin).unwrap();
}
