mod catalog;

use std::sync::LazyLock;

use catalog::JsonCatalog;
use esf::{
    AttributeRecord, Catalog, Entry, EsfFit, Location, MutaplasmidRecord, State, TypeRecord,
};

static CATALOG: LazyLock<JsonCatalog> = LazyLock::new(|| JsonCatalog::new(&catalog::path()));

const RIFTER: u32 = 587;
const AUTOCANNON: u32 = 2889;
const EMP: u32 = 21898;
const SHIELD_EXTENDER: u32 = 3831;
const SCRAMBLER: u32 = 448;
const DAMAGE_CONTROL: u32 = 2048;
const HOBGOBLIN: u32 = 2456;
const DECAYED_DAMAGE_CONTROL_MUTAPLASMID: u32 = 52224;
const AVATAR: u32 = 11567;
const AVATAR_UNPUBLISHED: u32 = 50097;
const SMALL_SECURE_CONTAINER: u32 = 3467;

fn entry(type_id: u32) -> Entry {
    Entry {
        type_id: Some(type_id),
        ..Entry::default()
    }
}

fn rifter(entries: Vec<Entry>) -> EsfFit {
    EsfFit {
        hull: Some(RIFTER),
        entries,
        ..EsfFit::default()
    }
}

fn named(name: &str) -> EsfFit {
    EsfFit {
        name: Some(name.to_string()),
        ..rifter(vec![])
    }
}

fn save(fits: &[EsfFit]) -> Result<String, esf::Error> {
    esf::save(fits, &*CATALOG)
}

fn load(data: impl AsRef<[u8]>) -> Result<Vec<EsfFit>, esf::Error> {
    esf::load(data, &*CATALOG)
}

#[track_caller]
fn assert_error<T: std::fmt::Debug>(result: Result<T, esf::Error>, message: &str) {
    let error = result.expect_err("an error");
    assert!(
        error.to_string().contains(message),
        "{error:?} does not say {message:?}"
    );
}

#[test]
fn save_canonical() {
    let fit = EsfFit {
        name: Some("Shield Buffer".to_string()),
        ..rifter(vec![
            entry(DAMAGE_CONTROL),
            Entry {
                charge: Some(EMP),
                ..entry(AUTOCANNON)
            },
            entry(SHIELD_EXTENDER),
            Entry {
                charge: Some(EMP),
                ..entry(AUTOCANNON)
            },
            Entry {
                location: Some(Location::High),
                ..Entry::default()
            },
            Entry {
                state: Some(State::Off),
                ..entry(SCRAMBLER)
            },
            Entry {
                count: Some(2),
                ..entry(HOBGOBLIN)
            },
            Entry {
                count: Some(100),
                location: Some(Location::Cargo),
                ..entry(EMP)
            },
        ])
    };
    assert_eq!(
        save(&[fit]).unwrap(),
        "%esf/1\n\
         Rifter \"Shield Buffer\"\n\
         \n\
         2x 200mm AutoCannon II :Republic Fleet EMP S\n\
         \n\
         Medium Shield Extender II\n\
         Warp Scrambler II !off\n\
         \n\
         Damage Control II\n\
         \n\
         2x Hobgoblin II\n\
         \n\
         100x Republic Fleet EMP S\n"
    );
}

#[test]
fn load_canonical() {
    let fit = rifter(vec![
        entry(AUTOCANNON),
        entry(AUTOCANNON),
        Entry {
            location: Some(Location::Cargo),
            ..entry(EMP)
        },
    ]);
    let want = rifter(vec![
        Entry {
            count: Some(2),
            ..entry(AUTOCANNON)
        },
        entry(EMP),
    ]);
    assert_eq!(load(save(&[fit]).unwrap()).unwrap(), [want]);
}

#[test]
fn load_text() {
    assert_eq!(load("%esf/1\nRifter\n").unwrap(), [rifter(vec![])]);
    assert_eq!(load(b"%esf/1\nRifter\n").unwrap(), [rifter(vec![])]);
}

#[test]
fn mutaplasmid_rolls() {
    let mutated = Entry {
        mutaplasmid: Some(DECAYED_DAMAGE_CONTROL_MUTAPLASMID),
        overrides: vec![(50, 31.5)],
        ..entry(DAMAGE_CONTROL)
    };
    let loaded = load(save(&[rifter(vec![mutated])]).unwrap()).unwrap();
    let rolls = [(50, 31.5), (974, 0.6), (975, 0.6), (976, 0.6), (977, 0.6)];
    assert_eq!(loaded[0].entries[0].overrides, rolls);
}

#[test]
fn binary_round_trip() {
    let fits = [
        rifter(vec![Entry {
            fit_name: Some("Loot".to_string()),
            location: Some(Location::Cargo),
            ..entry(SMALL_SECURE_CONTAINER)
        }]),
        EsfFit {
            hull: Some(SMALL_SECURE_CONTAINER),
            name: Some("Loot".to_string()),
            mode: None,
            entries: vec![Entry {
                count: Some(10),
                ..entry(EMP)
            }],
        },
    ];
    let binary = esf::save_binary(&fits, &*CATALOG).unwrap();
    assert_eq!(load(binary).unwrap(), load(save(&fits).unwrap()).unwrap());
}

#[test]
fn save_invalid() {
    let invalid: [(&[EsfFit], &str); 16] = [
        (&[], "at least one fit"),
        (
            &[rifter(vec![Entry::default()])],
            "an empty slot must name its rack",
        ),
        (
            &[rifter(vec![Entry {
                count: Some(0),
                ..entry(AUTOCANNON)
            }])],
            "at least 1",
        ),
        (
            &[rifter(vec![Entry {
                charge: Some(EMP),
                charge_count: Some(0),
                ..entry(AUTOCANNON)
            }])],
            "at least 1",
        ),
        (&[rifter(vec![entry(999_999_999)])], "unknown item type ID"),
        (
            &[EsfFit {
                hull: Some(999_999_999),
                ..EsfFit::default()
            }],
            "unknown hull type ID",
        ),
        (
            &[EsfFit {
                mode: Some(999_999_999),
                ..rifter(vec![])
            }],
            "unknown tactical mode type ID",
        ),
        (
            &[rifter(vec![Entry {
                charge: Some(999_999_999),
                ..entry(AUTOCANNON)
            }])],
            "unknown charge type ID",
        ),
        (
            &[rifter(vec![Entry {
                mutaplasmid: Some(999_999_999),
                ..entry(DAMAGE_CONTROL)
            }])],
            "unknown mutaplasmid type ID",
        ),
        (
            &[rifter(vec![Entry {
                overrides: vec![(999_999, 1.0)],
                ..entry(DAMAGE_CONTROL)
            }])],
            "unknown attribute ID",
        ),
        (
            &[rifter(vec![Entry {
                location: Some(Location::High),
                ..entry(AUTOCANNON)
            }])],
            "only for an empty slot",
        ),
        (
            &[rifter(vec![Entry {
                charge_count: Some(10),
                ..entry(AUTOCANNON)
            }])],
            "without a charge",
        ),
        (
            &[rifter(vec![Entry {
                overrides: vec![(50, f64::NAN)],
                ..entry(DAMAGE_CONTROL)
            }])],
            "not finite",
        ),
        (
            &[rifter(vec![Entry {
                overrides: vec![(50, f64::INFINITY)],
                ..entry(DAMAGE_CONTROL)
            }])],
            "not finite",
        ),
        (
            &[rifter(vec![Entry {
                location: Some(Location::Cargo),
                ..entry(AVATAR_UNPUBLISHED)
            }])],
            "not what its name resolves to",
        ),
        (&[named("Two\nlines")], "CR or LF"),
    ];
    for (fits, message) in invalid {
        assert_error(save(fits), message);
    }
}

#[test]
fn save_contains_itself() {
    let fit = EsfFit {
        name: Some("Buffer".to_string()),
        ..rifter(vec![Entry {
            fit_name: Some("Buffer".to_string()),
            ..entry(RIFTER)
        }])
    };
    assert_error(save(&[fit]), "cannot contain itself");
}

#[test]
fn error_location() {
    let error = save(&[
        rifter(vec![]),
        rifter(vec![entry(AUTOCANNON), entry(999_999_999)]),
    ])
    .unwrap_err();
    assert_eq!(
        (error.fit, error.entry, error.line),
        (Some(2), Some(2), None)
    );
    assert!(error.to_string().starts_with("fit 2, entry 2: "));
}

#[test]
fn error_location_of_a_fit() {
    let error = save(&[
        rifter(vec![]),
        EsfFit {
            hull: Some(999_999_999),
            ..EsfFit::default()
        },
    ])
    .unwrap_err();
    assert_eq!((error.fit, error.entry, error.line), (Some(2), None, None));
    assert!(error.to_string().starts_with("fit 2: "));
}

#[test]
fn error_location_of_a_resolved_line() {
    let stored = Entry {
        state: Some(State::Off),
        location: Some(Location::Cargo),
        ..entry(EMP)
    };
    let error = save(&[rifter(vec![entry(AUTOCANNON), stored])]).unwrap_err();
    assert_eq!((error.fit, error.entry), (Some(1), Some(2)));
    let error = save(&[EsfFit {
        mode: Some(AUTOCANNON),
        ..rifter(vec![])
    }])
    .unwrap_err();
    assert_eq!((error.fit, error.entry), (Some(1), None));
}

#[test]
fn error_location_of_text() {
    let error = load("%esf/1\nRifter\nNo Such Module\n").unwrap_err();
    assert_eq!((error.fit, error.entry, error.line), (None, None, Some(3)));
    assert_eq!(error.to_string(), "line 3: unknown type \"No Such Module\"");
}

#[test]
fn empty_name_is_no_name() {
    assert_eq!(save(&[named("")]).unwrap(), "%esf/1\nRifter\n");
    let empty = Entry {
        fit_name: Some(String::new()),
        ..entry(DAMAGE_CONTROL)
    };
    assert_eq!(
        save(&[rifter(vec![empty])]).unwrap(),
        "%esf/1\nRifter\n\nDamage Control II\n"
    );
}

#[test]
fn name_zero_is_a_name() {
    assert_eq!(save(&[named("0")]).unwrap(), "%esf/1\nRifter \"0\"\n");
}

#[test]
fn shared_name_published_wins() {
    let fit = rifter(vec![Entry {
        location: Some(Location::Cargo),
        ..entry(AVATAR)
    }]);
    assert!(save(&[fit]).unwrap().ends_with("Avatar\n"));
}

#[test]
fn load_invalid() {
    let invalid: [(&[u8], &str); 4] = [
        (b"", "the document is empty"),
        (
            b"%esf/1\nRifter\xED\xA0\x80\n",
            "not valid UTF-8 at byte 13",
        ),
        (b"\x10\x01", "at least one fit"),
        (
            b"%esf/1\nRifter\n4294967295x Damage Control II\n4294967295x Damage Control II\n",
            "too large",
        ),
    ];
    for (data, message) in invalid {
        assert_error(load(data), message);
    }
}

#[test]
fn load_invalid_binary() {
    let invalid: [(&[u8], &str); 5] = [
        (b"\x0A", "cut short"),
        (b"\x0A\x80", "cut short"),
        (
            b"\x10\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x01",
            "varint is too long",
        ),
        (b"\x80\x80\x80\x80\x80\x01", "bad key 34359738368"),
        (
            b"\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x01",
            "bad key 18446744073709551615",
        ),
    ];
    for (data, message) in invalid {
        assert_error(load(data), message);
    }
}

#[test]
fn binary_skips_unknown_document_fields() {
    let fit = esf::save_binary(&[rifter(vec![])], &*CATALOG).unwrap();
    let unknown: [&[u8]; 4] = [
        b"\x11\x00\x00\x00\x00\x00\x00\x00\x00",
        b"\x15\x00\x00\x00\x00",
        b"\x1B\x08\x01\x23\x24\x1C",
        b"\x18\x05",
    ];
    for prefix in unknown {
        assert_eq!(load([prefix, &fit].concat()).unwrap(), [rifter(vec![])]);
    }
    assert_error(
        load([b"\x1B\x23\x1C", &fit[..]].concat()),
        "unmatched end group",
    );
    assert_error(load([b"\x1C", &fit[..]].concat()), "unmatched end group");
}

/// The bytes of a document with one fit, given the bytes of that fit after its version.
fn document(fit: &[u8]) -> Vec<u8> {
    let fit = [b"\x08\x01", fit].concat();
    [&[0x0A, fit.len() as u8][..], &fit].concat()
}

/// The bytes of a document with one Rifter, given the bytes of its one entry.
fn document_with_entry(entry: &[u8]) -> Vec<u8> {
    document(&[&b"\x10\xCB\x04\x2A"[..], &[entry.len() as u8], entry].concat())
}

#[test]
fn binary_entries() {
    let packed = [
        b"\x08\x80\x10\x3A\x01\x32\x42\x08",
        &31.5f64.to_le_bytes()[..],
    ]
    .concat();
    let unpacked = [b"\x08\x80\x10\x38\x32\x41", &31.5f64.to_le_bytes()[..]].concat();
    for bytes in [packed, unpacked] {
        let fits = load(document_with_entry(&bytes)).unwrap();
        assert_eq!(fits[0].entries[0].overrides, [(50, 31.5)]);
    }
    let fits = load(document_with_entry(b"\x08\xC9\x16\x48\x01")).unwrap();
    assert_eq!(fits[0].entries[0].state, Some(State::Off));
    let fits = load(document_with_entry(b"\x08\x80\x10\x48\x00\x50\x00\x1A\x00")).unwrap();
    assert_eq!(fits[0].entries[0], entry(DAMAGE_CONTROL));
}

#[test]
fn binary_invalid() {
    let invalid: [(Vec<u8>, &str); 16] = [
        (b"\x08\x01".to_vec(), "wrong wire type"),
        (
            document(b"\x10\x80\x80\x80\x80\x10"),
            "Fit.hull is out of range",
        ),
        (document(b"\x1A\x01\xFF"), "Fit.name is not valid UTF-8"),
        (
            document(b"\x30\x01"),
            "Fit has a field that is not in the schema",
        ),
        (
            document(b"\x12\x00"),
            "Fit has a field that is not in the schema",
        ),
        (b"\x0A\x02\x08\x02".to_vec(), "unsupported Fit.version 2"),
        (b"\x0A\x00".to_vec(), "unsupported Fit.version 0"),
        (
            document_with_entry(b"\x58\x01"),
            "Entry has a field that is not in the schema",
        ),
        (
            document_with_entry(b"\x1A\x01\xFF"),
            "Entry.fit_name is not valid UTF-8",
        ),
        (document_with_entry(b"\x48\x04"), "unknown State value 4"),
        (
            document_with_entry(b"\x50\x1C"),
            "unknown Location value 28",
        ),
        (document_with_entry(b"\x38\x32"), "different lengths"),
        (document_with_entry(b"\x3A\x01\x80"), "cut short"),
        (
            document_with_entry(b"\x42\x01\x00"),
            "packed doubles cut short",
        ),
        (
            document_with_entry(&[b"\x08\x80\x10\x3A\x02\x32\x32\x42\x10", &[0; 16][..]].concat()),
            "overridden twice",
        ),
        (
            document_with_entry(b"\x08\xC9\x16\x10\x00"),
            "fit 1, entry 1: a count must be at least 1",
        ),
    ];
    for (data, message) in invalid {
        assert_error(load(data), message);
    }
}

struct Minimal;

impl Catalog for Minimal {
    fn type_by_name(&self, folded: &str) -> Option<&TypeRecord> {
        CATALOG.type_by_name(folded)
    }

    fn type_by_id(&self, type_id: u32) -> Option<&TypeRecord> {
        CATALOG.type_by_id(type_id)
    }

    fn attribute_by_name(&self, folded: &str) -> Option<&AttributeRecord> {
        CATALOG.attribute_by_name(folded)
    }

    fn attribute_by_id(&self, attribute_id: u32) -> Option<&AttributeRecord> {
        CATALOG.attribute_by_id(attribute_id)
    }
}

#[test]
fn catalog_without_optional_lookups() {
    assert_error(
        esf::load("%esf/1\nHecate /Defense\n", &Minimal),
        "no tactical modes",
    );
    assert_error(
        esf::load("%esf/1\nRifter\nDamage Control II +Decayed\n", &Minimal),
        "no mutaplasmid",
    );
}

struct WithoutRollAttributes;

impl Catalog for WithoutRollAttributes {
    fn type_by_name(&self, folded: &str) -> Option<&TypeRecord> {
        CATALOG.type_by_name(folded)
    }

    fn type_by_id(&self, type_id: u32) -> Option<&TypeRecord> {
        CATALOG.type_by_id(type_id)
    }

    fn attribute_by_name(&self, folded: &str) -> Option<&AttributeRecord> {
        CATALOG.attribute_by_name(folded)
    }

    fn attribute_by_id(&self, _attribute_id: u32) -> Option<&AttributeRecord> {
        None
    }

    fn mutaplasmids(&self, type_id: u32) -> &[MutaplasmidRecord] {
        CATALOG.mutaplasmids(type_id)
    }
}

#[test]
fn catalog_missing_roll_attribute() {
    let text = "%esf/1\nRifter\nDamage Control II +Decayed\n";
    assert_error(
        esf::load(text, &WithoutRollAttributes),
        "the catalog has no attribute 50",
    );
}

struct NewlineNames(TypeRecord);

impl Catalog for NewlineNames {
    fn type_by_name(&self, folded: &str) -> Option<&TypeRecord> {
        CATALOG.type_by_name(folded)
    }

    fn type_by_id(&self, _type_id: u32) -> Option<&TypeRecord> {
        Some(&self.0)
    }

    fn attribute_by_name(&self, folded: &str) -> Option<&AttributeRecord> {
        CATALOG.attribute_by_name(folded)
    }

    fn attribute_by_id(&self, attribute_id: u32) -> Option<&AttributeRecord> {
        CATALOG.attribute_by_id(attribute_id)
    }
}

#[test]
fn catalog_name_with_newline() {
    let hull = CATALOG.type_by_id(RIFTER).unwrap();
    let catalog = NewlineNames(TypeRecord {
        name: "Rif\nter".to_string(),
        ..hull.clone()
    });
    let error = esf::save(&[rifter(vec![])], &catalog).unwrap_err();
    assert_eq!(
        error.to_string(),
        "fit 1: the name of hull type ID 587 contains CR or LF"
    );
}

#[test]
fn base64url() {
    let data = esf::save_binary(&[rifter(vec![entry(AUTOCANNON)])], &*CATALOG).unwrap();
    let url = esf::to_base64url(&data);
    assert!(!url.contains('='));
    assert_eq!(esf::from_base64url(&url).unwrap(), data);
    for invalid in ["not base64url!", "AA==", "AAAAA"] {
        assert_error(esf::from_base64url(invalid), "not valid base64url");
    }
}

#[test]
fn is_text() {
    assert!(esf::is_text("%esf/1\n"));
    assert!(esf::is_text(b" %esf/1\n"));
    assert!(!esf::is_text(b"\x0a\x02"));
    assert!(!esf::is_text(b""));
}

#[test]
fn fold() {
    assert_eq!(esf::fold("Damage Control II"), "damage control ii");
    assert_eq!(esf::fold("Straße"), "straße");
}

#[test]
fn display() {
    assert_eq!(State::Heat.to_string(), "heat");
    assert_eq!(Location::Infrastructure.to_string(), "infrastructure");
    assert!(Location::Svc.is_rack() && !Location::Svc.is_stored());
    assert!(Location::Bay.is_stored() && !Location::Bay.is_rack());
}

#[test]
fn many_fits_are_linear() {
    let count = 20_000;
    let fits: Vec<EsfFit> = (0..count)
        .map(|n| EsfFit {
            hull: Some(SMALL_SECURE_CONTAINER),
            name: Some(format!("Box {n}")),
            mode: None,
            entries: vec![Entry {
                fit_name: (n + 1 < count).then(|| format!("Box {}", n + 1)),
                ..entry(SMALL_SECURE_CONTAINER)
            }],
        })
        .collect();
    let started = std::time::Instant::now();
    let text = save(&fits).unwrap();
    assert_eq!(load(&text).unwrap().len(), count);
    assert!(
        started.elapsed().as_secs() < 10,
        "took {:?}",
        started.elapsed()
    );
}
