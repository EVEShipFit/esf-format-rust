//! The esf-format test cases, the way its runner checks them.

mod catalog;

use std::fs;
use std::path::{Path, PathBuf};

use catalog::JsonCatalog;

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

fn check_valid(path: &Path, catalog: &JsonCatalog) -> Result<(), String> {
    let canonical = fs::read_to_string(path.join("canonical.esf")).unwrap();
    let binary =
        esf::from_base64url(&fs::read_to_string(path.join("canonical.b64")).unwrap()).unwrap();
    let fits = esf::load(input(path), catalog).map_err(|e| format!("load: {e}"))?;
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
    }

    assert!(!valid.is_empty() && !invalid.is_empty());
    assert!(
        failures.is_empty(),
        "{} failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
