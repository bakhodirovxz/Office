//! Makes fixtures/long.docx (about 6 pages: Latin, Cyrillic and Uzbek paragraphs) with the bind API.
use wordcraft_bind::Editor;

fn main() {
    let out = std::env::args().nth(1).expect("usage: gen_fixture OUT.docx");
    let mut e = Editor::blank();
    let latin = "The quick brown fox jumps over the lazy dog while the committee reviews the quarterly personnel report. ";
    let cyr = "Салом дунё! Ходимларнинг йиллик ҳисоботи ва таътил жадвали тасдиқланди, ишчи гуруҳ учрашуви душанбага белгиланди. ";
    let uz = "Oʻzbekiston Respublikasi xodimlari uchun oʻgʻil bolalar taʼtili va mehnat shartnomasi boʻyicha maʼlumot. ";
    e.insert("Mobile B spike document").unwrap();
    e.run("format.bold", "{}").ok();
    e.run("text.newParagraph", "{}").unwrap();
    for i in 0..28 {
        let body = match i % 3 { 0 => latin, 1 => cyr, _ => uz };
        e.insert(&format!("{}. {}{}{}", i + 1, body, body, body)).unwrap();
        e.run("text.newParagraph", "{}").unwrap();
    }
    let bytes = e.save("long.docx").unwrap();
    std::fs::write(&out, &bytes).unwrap();
    let mut e2 = Editor::open("long.docx", &bytes).unwrap();
    println!("wrote {} bytes, {} pages", bytes.len(), e2.page_count());
}
