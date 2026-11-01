//! The program the esf-format test runner calls: program check|canonical|binary.

#[path = "../tests/catalog/mod.rs"]
mod catalog;

use std::io::{Read, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_default();
    let catalog = catalog::JsonCatalog::new(&catalog::path());
    let mut input = Vec::new();
    std::io::stdin()
        .read_to_end(&mut input)
        .expect("stdin is readable");

    let output = esf::load(&input, &catalog).and_then(|fits| match command.as_str() {
        "canonical" => esf::save(&fits, &catalog).map(String::into_bytes),
        "binary" => esf::save_binary(&fits, &catalog),
        _ => Ok(Vec::new()),
    });
    match output {
        Ok(output) => {
            std::io::stdout()
                .write_all(&output)
                .expect("stdout is writable");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("invalid: {error}");
            ExitCode::FAILURE
        }
    }
}
