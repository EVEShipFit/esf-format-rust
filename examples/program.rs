//! The program the esf-format test runner calls: program check|canonical|binary|extra.

#[path = "../tests/catalog/mod.rs"]
mod catalog;

use std::io::{Read, Write};
use std::process::ExitCode;

fn extra_line(extra: &esf::Extra) -> String {
    let json = match extra {
        esf::Extra::Text {
            name,
            version,
            text,
        } => serde_json::json!({"name": name, "version": version, "text": text}),
        esf::Extra::Binary { field, bytes } => {
            serde_json::json!({"field": field, "bytes": esf::to_base64url(bytes)})
        }
    };
    format!("{json}\n")
}

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_default();
    let catalog = catalog::JsonCatalog::new(&catalog::path());
    let mut input = Vec::new();
    std::io::stdin()
        .read_to_end(&mut input)
        .expect("stdin is readable");

    let output = esf::load_document(&input, &catalog).and_then(|document| {
        let fits = document.fits;
        match command.as_str() {
            "canonical" => esf::save(&fits, &catalog).map(String::into_bytes),
            "binary" => esf::save_binary(&fits, &catalog),
            "extra" => Ok(document
                .extra
                .iter()
                .map(extra_line)
                .collect::<String>()
                .into_bytes()),
            _ => Ok(Vec::new()),
        }
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
