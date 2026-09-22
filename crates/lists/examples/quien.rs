//! Who is behind a name, and from where, straight from the lists.
//!
//! Maintaining `empresas.txt` by eye does not work: a section that is never matched, a country
//! written in the wrong place or a domain that belongs to a company we already have under
//! another name all look fine in the file and only show up in the panel. This asks the library
//! the same question the panel asks.
//!
//! ```text
//! cargo run -p guardiana-lists --example quien -- hotjar.com teads.tv
//! cargo run -p guardiana-lists --example quien            (las que se quedan sin país o sin ciudad)
//! ```
fn main() {
    let nombres: Vec<String> = std::env::args().skip(1).collect();
    if nombres.is_empty() {
        println!("Empresas sin país o sin ciudad, que es una decisión y no un olvido:\n");
        let mut sin: Vec<&str> = guardiana_lists::EMPRESAS
            .lines()
            .filter_map(|l| l.split('#').next().unwrap_or("").trim().strip_prefix('@'))
            .filter(|s| s.split('|').nth(2).is_none_or(str::is_empty))
            .collect();
        sin.sort_unstable();
        for empresa in sin {
            println!("  {empresa}");
        }
        println!("\nPara preguntar por un nombre: … --example quien -- ejemplo.com");
        return;
    }
    for n in nombres {
        let empresa = guardiana_lists::company_of(&n).unwrap_or("—");
        let pais = guardiana_lists::country_of(&n).unwrap_or("—");
        // La ciudad también: es lo que el panel enseña debajo del país, y mirarla aquí es la
        // única manera de ver que una sección mal escrita se quedó sin ella.
        let ciudad = guardiana_lists::city_of(&n).unwrap_or("—");
        println!("{n:28} {empresa:26} {pais:4} {ciudad}");
    }
}
