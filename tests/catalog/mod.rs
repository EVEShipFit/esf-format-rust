//! A Catalog read from the esf-format test catalog.

use std::collections::HashMap;

use esf::{AttributeRecord, Catalog, Kind, MutaplasmidRecord, TypeRecord, fold};
use serde_json::Value;

pub struct JsonCatalog {
    types: HashMap<u32, TypeRecord>,
    attributes: HashMap<u32, AttributeRecord>,
    type_names: HashMap<String, u32>,
    attribute_names: HashMap<String, u32>,
    modes: HashMap<u32, Vec<TypeRecord>>,
    mutaplasmids: HashMap<u32, Vec<MutaplasmidRecord>>,
}

fn id(value: &Value) -> u32 {
    value.as_u64().unwrap().try_into().unwrap()
}

fn kind(name: &str) -> Kind {
    match name {
        "hull" => Kind::Hull,
        "container" => Kind::Container,
        "mode" => Kind::Mode,
        "sub" => Kind::Sub,
        "high" => Kind::High,
        "mid" => Kind::Mid,
        "low" => Kind::Low,
        "rig" => Kind::Rig,
        "svc" => Kind::Svc,
        "drone" => Kind::Drone,
        "fighter" => Kind::Fighter,
        "charge" => Kind::Charge,
        "implant" => Kind::Implant,
        "booster" => Kind::Booster,
        _ => Kind::Other,
    }
}

/// Where names are shared, published wins, then the lowest ID (§4.2).
fn by_name(entries: &[Value]) -> HashMap<String, u32> {
    let mut sorted: Vec<&Value> = entries.iter().collect();
    sorted.sort_by_key(|e| (!e["published"].as_bool().unwrap(), id(&e["id"])));
    let mut index = HashMap::new();
    for e in sorted {
        index
            .entry(fold(e["name"].as_str().unwrap()))
            .or_insert(id(&e["id"]));
    }
    index
}

impl JsonCatalog {
    pub fn new(path: &str) -> JsonCatalog {
        let data: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let types: HashMap<u32, TypeRecord> = data["types"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| {
                let record = TypeRecord {
                    id: id(&t["id"]),
                    name: t["name"].as_str().unwrap().to_string(),
                    kind: kind(t["kind"].as_str().unwrap()),
                    active: t["active"].as_bool().unwrap(),
                    capacity: t["capacity"].as_f64().unwrap(),
                    volume: t["volume"].as_f64().unwrap(),
                    squadron_size: t.get("squadron_size").map(id),
                };
                (record.id, record)
            })
            .collect();
        let attributes = data["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                let name = a["name"].as_str().unwrap().to_string();
                (
                    id(&a["id"]),
                    AttributeRecord {
                        id: id(&a["id"]),
                        name,
                    },
                )
            })
            .collect();
        let modes = data["modes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| {
                let modes = m["modes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|t| types[&id(t)].clone());
                (id(&m["hull"]), modes.collect())
            })
            .collect();
        let mut mutaplasmids: HashMap<u32, Vec<MutaplasmidRecord>> = HashMap::new();
        for m in data["mutaplasmids"].as_array().unwrap() {
            let rolls = m["rolls"].as_array().unwrap().iter();
            let rolls = rolls.map(|r| (id(&r[0]), r[1].as_f64().unwrap())).collect();
            let mutaplasmid = types[&id(&m["mutaplasmid"])].clone();
            let record = MutaplasmidRecord { mutaplasmid, rolls };
            mutaplasmids.entry(id(&m["base"])).or_default().push(record);
        }
        JsonCatalog {
            types,
            attributes,
            type_names: by_name(data["types"].as_array().unwrap()),
            attribute_names: by_name(data["attributes"].as_array().unwrap()),
            modes,
            mutaplasmids,
        }
    }
}

impl Catalog for JsonCatalog {
    fn type_by_name(&self, folded: &str) -> Option<&TypeRecord> {
        self.types.get(self.type_names.get(folded)?)
    }

    fn type_by_id(&self, type_id: u32) -> Option<&TypeRecord> {
        self.types.get(&type_id)
    }

    fn attribute_by_name(&self, folded: &str) -> Option<&AttributeRecord> {
        self.attributes.get(self.attribute_names.get(folded)?)
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

/// The test catalog: `$ESF_CATALOG`, or the one in a clone of esf-format.
pub fn path() -> String {
    std::env::var("ESF_CATALOG").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/esf-format/tests/catalog.json").to_string()
    })
}
