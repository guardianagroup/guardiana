//! Which registered data brokers a list of names belongs to: one name per line on stdin, and for
//! each one that a registrant owns, `name<TAB>registrant<TAB>country<TAB>declared`. For sizing a
//! change of `corredores.txt` against a real extract before shipping it.
use std::io::BufRead;

fn main() {
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(name) = line else { break };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        if let Some(b) = guardiana_lists::data_broker_of(name) {
            println!(
                "{name}\t{}\t{}\t{}",
                b.name,
                b.country.unwrap_or(""),
                b.declared
            );
        }
    }
}
