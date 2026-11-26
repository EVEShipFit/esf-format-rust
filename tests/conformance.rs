//! The esf-format test cases, the way its runner checks them.

mod catalog;

use std::fs;
use std::path::{Path, PathBuf};

use catalog::JsonCatalog;
use esf::Extra;

fn cases(root: &Path, kind: &str) -> Vec<PathBuf> {
    let mut cases: Vec<PathBuf> = fs::read_dir(root.join(kind))
        .unwrap_or_else(|_| {
            panic!(
                "no test cases in {}: clone esf-format first",
                root.display()
            )
        })
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    cases.sort();
    cases
}

fn input(path: &Path) -> Vec<u8> {
    match fs::read(path.join("input.esf")) {
        Ok(text) => text,
        Err(_) => {
            esf::from_base64url(&fs::read_to_string(path.join("input.b64")).unwrap()).unwrap()
        }
    }
}

fn expected_extra(path: &Path) -> Vec<Extra> {
    let Ok(text) = fs::read_to_string(path.join("extra.toml")) else {
        return Vec::new();
    };
    let table: toml::Table = text.parse().unwrap();
    let int = |value: &toml::Value| value.as_integer().unwrap().try_into().unwrap();
    table["extra"]
        .as_array()
        .unwrap()
        .iter()
        .map(|extra| match extra.get("field") {
            Some(field) => Extra::Binary {
                field: int(field),
                bytes: esf::from_base64url(extra["bytes"].as_str().unwrap()).unwrap(),
            },
            None => Extra::Text {
                name: extra["name"].as_str().unwrap().to_string(),
                version: int(&extra["version"]),
                text: extra["text"].as_str().unwrap().to_string(),
            },
        })
        .collect()
}

fn check_valid(path: &Path, catalog: &JsonCatalog) -> Result<(), String> {
    let canonical = fs::read_to_string(path.join("canonical.esf")).unwrap();
    let binary =
        esf::from_base64url(&fs::read_to_string(path.join("canonical.b64")).unwrap()).unwrap();
    let fits = esf::load(input(path), catalog).map_err(|e| format!("load: {e}"))?;
    let document =
        esf::load_document(input(path), catalog).map_err(|e| format!("load_document: {e}"))?;
    if document.fits != fits {
        return Err(format!("load_document: got fits {:?}", document.fits));
    }
    if document.extra != expected_extra(path) {
        return Err(format!("load_document: got extra {:?}", document.extra));
    }
    let round_trip =
        |data: &[u8]| esf::load(data, catalog).and_then(|fits| esf::save(&fits, catalog));

    let checks = [
        (
            "canonical of input",
            esf::save(&fits, catalog).map(String::into_bytes),
            canonical.as_bytes(),
        ),
        ("binary of input", esf::save_binary(&fits, catalog), &binary),
        (
            "canonical of canonical.esf",
            round_trip(canonical.as_bytes()).map(String::into_bytes),
            canonical.as_bytes(),
        ),
        (
            "canonical of canonical.b64",
            round_trip(&binary).map(String::into_bytes),
            canonical.as_bytes(),
        ),
    ];
    for (what, got, want) in checks {
        match got {
            Ok(got) if got == want => {}
            Ok(got) => return Err(format!("{what}: got {:?}", String::from_utf8_lossy(&got))),
            Err(e) => return Err(format!("{what}: {e}")),
        }
    }
    Ok(())
}

#[test]
fn test_cases() {
    let path = catalog::path();
    let catalog = JsonCatalog::new(&path);
    let root = Path::new(&path).parent().unwrap();
    let mut failures = Vec::new();

    let valid = cases(root, "valid");
    for case in &valid {
        if let Err(why) = check_valid(case, &catalog) {
            failures.push(format!("{}: {why}", case.display()));
        }
    }
    let invalid = cases(root, "invalid");
    for case in &invalid {
        if let Ok(fits) = esf::load(input(case), &catalog) {
            failures.push(format!("{}: accepted {fits:?}", case.display()));
        }
        if let Ok(document) = esf::load_document(input(case), &catalog) {
            failures.push(format!(
                "{}: load_document accepted {document:?}",
                case.display()
            ));
        }
    }

    assert!(!valid.is_empty() && !invalid.is_empty());
    assert!(
        failures.is_empty(),
        "{} failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
