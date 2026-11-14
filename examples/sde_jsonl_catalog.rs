//! A Catalog read from the official SDE in JSONL.
//!
//! `cargo run --example sde_jsonl_catalog -- <SDE directory> < fit.esf` prints the fit in
//! canonical form.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

use esf::{AttributeRecord, Catalog, Kind, MutaplasmidRecord, TypeRecord, sde};
use serde_json::Value;

/// A Catalog read from the official SDE in JSONL, unpacked into one directory.
pub struct SdeJsonlCatalog {
    types: HashMap<u32, TypeRecord>,
    attributes: HashMap<u32, AttributeRecord>,
    type_names: HashMap<String, u32>,
    attribute_names: HashMap<String, u32>,
    modes: HashMap<u32, Vec<TypeRecord>>,
    mutaplasmids: HashMap<u32, Vec<MutaplasmidRecord>>,
}

struct Named {
    id: u32,
    name: String,
    published: bool,
}

struct Dogma {
    effects: Vec<u32>,
    squadron_size: Option<u32>,
    rollable: HashMap<u32, f64>,
}

struct Mutaplasmid {
    id: u32,
    bases: BTreeSet<u32>,
    attributes: Vec<u32>,
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn rows(dir: &Path, name: &str, mut each: impl FnMut(&Value) -> io::Result<()>) -> io::Result<()> {
    for line in BufReader::new(File::open(dir.join(name))?).lines() {
        each(&serde_json::from_str(&line?)?)?;
    }
    Ok(())
}

fn int(row: &Value, key: &str) -> io::Result<u32> {
    row[key]
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| invalid(format!("no {key} in {row}")))
}

fn key(row: &Value) -> io::Result<u32> {
    int(row, "_key")
}

fn float(row: &Value, key: &str) -> f64 {
    row[key].as_f64().unwrap_or(0.0)
}

fn published(row: &Value) -> bool {
    row["published"].as_bool().unwrap_or(false)
}

fn list<'a>(row: &'a Value, key: &str) -> &'a [Value] {
    row[key].as_array().map_or(&[], Vec::as_slice)
}

fn names(entries: &[Named]) -> HashMap<String, u32> {
    sde::name_index(entries, |e| e.id, |e| &e.name, |e| e.published)
}

impl SdeJsonlCatalog {
    /// Reads the SDE files in `dir`.
    pub fn new(dir: &Path) -> io::Result<SdeJsonlCatalog> {
        let mut category_of_group = HashMap::new();
        rows(dir, "groups.jsonl", |g| {
            category_of_group.insert(key(g)?, int(g, "categoryID")?);
            Ok(())
        })?;

        let mut effect_category = HashMap::new();
        rows(dir, "dogmaEffects.jsonl", |e| {
            effect_category.insert(key(e)?, int(e, "effectCategoryID").unwrap_or(0));
            Ok(())
        })?;

        let mut mutaplasmid_rows = Vec::new();
        rows(dir, "dynamicItemAttributes.jsonl", |m| {
            let mut bases = BTreeSet::new();
            for mapping in list(m, "inputOutputMapping") {
                for base in list(mapping, "applicableTypes") {
                    let base = base.as_u64().and_then(|b| u32::try_from(b).ok());
                    bases.insert(base.ok_or_else(|| invalid(format!("bad base in {m}")))?);
                }
            }
            let attributes = list(m, "attributeIDs").iter().map(key);
            mutaplasmid_rows.push(Mutaplasmid {
                id: key(m)?,
                bases,
                attributes: attributes.collect::<io::Result<_>>()?,
            });
            Ok(())
        })?;

        let rollable: HashSet<u32> = mutaplasmid_rows
            .iter()
            .flat_map(|m| m.attributes.iter().copied())
            .collect();
        let mut dogma = HashMap::new();
        rows(dir, "typeDogma.jsonl", |row| {
            let effects = list(row, "dogmaEffects").iter().map(|e| int(e, "effectID"));
            let mut values = HashMap::new();
            for a in list(row, "dogmaAttributes") {
                values.insert(int(a, "attributeID")?, float(a, "value"));
            }
            let squadron_size = sde::squadron_size(&values);
            values.retain(|id, _| rollable.contains(id));
            let effects = effects.collect::<io::Result<_>>()?;
            dogma.insert(
                key(row)?,
                Dogma {
                    effects,
                    squadron_size,
                    rollable: values,
                },
            );
            Ok(())
        })?;

        let mut types = HashMap::new();
        let mut type_rows = Vec::new();
        let mut ships = Vec::new();
        rows(dir, "types.jsonl", |t| {
            let Some(name) = t["name"]["en"].as_str() else {
                return Ok(());
            };
            let id = key(t)?;
            let group_id = int(t, "groupID")?;
            let category_id = *category_of_group
                .get(&group_id)
                .ok_or_else(|| invalid(format!("no group {group_id}")))?;
            let d = dogma.get(&id);
            let effects = d.map_or(&[][..], |d| d.effects.as_slice());
            let categories = effects
                .iter()
                .map(|e| (*e, effect_category.get(e).copied().unwrap_or(0)));
            types.insert(
                id,
                TypeRecord {
                    id,
                    name: name.to_string(),
                    kind: sde::kind(category_id, group_id, effects),
                    active: sde::active(categories),
                    capacity: float(t, "capacity"),
                    volume: float(t, "volume"),
                    squadron_size: d.and_then(|d| d.squadron_size),
                },
            );
            let named = || Named {
                id,
                name: name.to_string(),
                published: published(t),
            };
            if category_id == 6 {
                ships.push(named());
            }
            type_rows.push(named());
            Ok(())
        })?;

        let mut attributes = HashMap::new();
        let mut attribute_rows = Vec::new();
        let mut defaults = HashMap::new();
        rows(dir, "dogmaAttributes.jsonl", |a| {
            let id = key(a)?;
            let name = a["name"]
                .as_str()
                .ok_or_else(|| invalid(format!("no name in {a}")))?;
            attributes.insert(
                id,
                AttributeRecord {
                    id,
                    name: name.to_string(),
                },
            );
            attribute_rows.push(Named {
                id,
                name: name.to_string(),
                published: published(a),
            });
            defaults.insert(id, float(a, "defaultValue"));
            Ok(())
        })?;

        let ship_names = names(&ships);
        let mut modes: HashMap<u32, Vec<TypeRecord>> = HashMap::new();
        for mode in types.values().filter(|t| t.kind == Kind::Mode) {
            if let Some(hull) = sde::mode_hull(&mode.name, &ship_names) {
                modes.entry(hull).or_default().push(mode.clone());
            }
        }
        for list in modes.values_mut() {
            list.sort_by_key(|t| t.id);
        }

        mutaplasmid_rows.sort_by_key(|m| m.id);
        let mut mutaplasmids: HashMap<u32, Vec<MutaplasmidRecord>> = HashMap::new();
        for m in &mutaplasmid_rows {
            let mutaplasmid = types
                .get(&m.id)
                .ok_or_else(|| invalid(format!("no type {}", m.id)))?;
            for &base in &m.bases {
                let values = dogma.get(&base).map(|d| &d.rollable);
                let rolls = m.attributes.iter().map(|&a| {
                    let value = values.and_then(|v| v.get(&a)).or(defaults.get(&a));
                    (a, value.copied().unwrap_or(0.0))
                });
                mutaplasmids
                    .entry(base)
                    .or_default()
                    .push(MutaplasmidRecord {
                        mutaplasmid: mutaplasmid.clone(),
                        rolls: rolls.collect(),
                    });
            }
        }

        Ok(SdeJsonlCatalog {
            types,
            attributes,
            type_names: names(&type_rows),
            attribute_names: names(&attribute_rows),
            modes,
            mutaplasmids,
        })
    }
}

impl Catalog for SdeJsonlCatalog {
    fn type_by_name(&self, folded_name: &str) -> Option<&TypeRecord> {
        self.types.get(self.type_names.get(folded_name)?)
    }

    fn type_by_id(&self, type_id: u32) -> Option<&TypeRecord> {
        self.types.get(&type_id)
    }

    fn attribute_by_name(&self, folded_name: &str) -> Option<&AttributeRecord> {
        self.attributes.get(self.attribute_names.get(folded_name)?)
    }

    fn attribute_by_id(&self, attribute_id: u32) -> Option<&AttributeRecord> {
        self.attributes.get(&attribute_id)
    }

    fn modes(&self, hull_id: u32) -> &[TypeRecord] {
        self.modes.get(&hull_id).map_or(&[], Vec::as_slice)
    }

    fn mutaplasmids(&self, type_id: u32) -> &[MutaplasmidRecord] {
        self.mutaplasmids.get(&type_id).map_or(&[], Vec::as_slice)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args()
        .nth(1)
        .ok_or("usage: sde_jsonl_catalog <SDE directory> < fit.esf")?;
    let catalog = SdeJsonlCatalog::new(Path::new(&dir))?;
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input)?;
    print!("{}", esf::save(&esf::load(&input, &catalog)?, &catalog)?);
    Ok(())
}
