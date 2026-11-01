# esf

[![crates.io](https://img.shields.io/crates/v/esf.svg)](https://crates.io/crates/esf)
[![docs.rs](https://img.shields.io/docsrs/esf)](https://docs.rs/esf)
[![CI](https://github.com/EVEShipFit/esf-format-rust/actions/workflows/test.yml/badge.svg)](https://github.com/EVEShipFit/esf-format-rust/actions/workflows/test.yml)

[![Discord](https://img.shields.io/badge/Discord-Join-5865F2?logo=discord&logoColor=white)](https://discord.gg/S5V5BkvNf7)

Read and write [esf/1](https://github.com/EVEShipFit/esf-format), the
plain-text format for EVE Online ship fits. Text, canonical and binary forms.
No dependencies.

```sh
cargo add esf
```

## Usage

```rust
use esf::{Catalog, Entry, EsfFit, Location, State};

fn example(catalog: &dyn Catalog) -> Result<(), esf::Error> {
    let item = |type_id| Entry { type_id: Some(type_id), ..Entry::default() };
    let fit = EsfFit {
        hull: Some(587), // Rifter
        name: Some("Shield Buffer".to_string()),
        mode: None,
        entries: vec![
            Entry { count: Some(3), charge: Some(21898), ..item(2889) }, // 3 guns, a full load of ammo each
            Entry { location: Some(Location::High), ..Entry::default() }, // an empty high slot
            Entry { state: Some(State::Off), ..item(448) }, // an offline module
            item(2048), // goes in its own rack
            Entry { count: Some(5), ..item(2456) }, // 5 drones
            Entry { count: Some(1000), location: Some(Location::Cargo), ..item(21898) },
        ],
    };

    let text = esf::save(&[fit.clone()], catalog)?; // canonical text
    let data = esf::save_binary(&[fit], catalog)?; // binary
    let fits = esf::load(&data, catalog)?; // text or binary

    let url = esf::to_base64url(&data); // the binary form in a URL
    let data = esf::from_base64url(&url)?;
    Ok(())
}
```

`save()` and `save_binary()` take entries in any order, and check every rule
of the spec. `load()` returns fits in canonical form. All three return an
`esf::Error` when a rule is broken. It says where: `line` for text, or `fit`
and `entry` (counting from 1) for fits.

## Fits

An `EsfFit` is one fit: `hull`, `name`, tactical `mode` and its `entries`.
`hull` is `None` for a fit without a ship. A document can hold more than one
fit.

Each `Entry` is one line of the fit. `None` means the default:

| field | what it is | `None` means |
| --- | --- | --- |
| `type_id` | The type ID. | An empty slot. |
| `count` | How many. | One; for fighters, a full squadron. |
| `fit_name` | The name of another fit in the list, which this ship or container carries. | Carries no fit. |
| `charge` | The charge loaded. | |
| `charge_count` | How many charges. | A full load. |
| `mutaplasmid` | The mutaplasmid applied. | |
| `overrides` | Attribute values by attribute ID. | |
| `state` | `State::Off`, `On` or `Heat`. | Active if the type can be, else on. |
| `location` | `Cargo`, `Bay` or another hold; a rack only for an empty slot. | Where the type goes by default. |

## The Catalog

The library holds no game data. You give it a `Catalog`: a type that looks
up types and attributes in your copy of the SDE.

```rust
use esf::{AttributeRecord, Catalog, MutaplasmidRecord, TypeRecord};

struct MyCatalog;

impl Catalog for MyCatalog {
    fn type_by_name(&self, folded: &str) -> Option<&TypeRecord> { todo!() }
    fn type_by_id(&self, type_id: u32) -> Option<&TypeRecord> { todo!() }
    fn attribute_by_name(&self, folded: &str) -> Option<&AttributeRecord> { todo!() }
    fn attribute_by_id(&self, attribute_id: u32) -> Option<&AttributeRecord> { todo!() }

    // Optional. Without them, no hull has tactical modes and no type has mutaplasmids.
    fn modes(&self, hull_id: u32) -> &[TypeRecord] { todo!() }
    fn mutaplasmids(&self, type_id: u32) -> &[MutaplasmidRecord] { todo!() }
}
```

Names are looked up folded: index them with `esf::fold(name)`. Where names
are shared, published wins, then the lowest ID (§4.2).
[tests/catalog/mod.rs](https://github.com/EVEShipFit/esf-format-rust/blob/main/tests/catalog/mod.rs)
is a complete example.

## Tests

The test cases live in [esf-format](https://github.com/EVEShipFit/esf-format):

```sh
git clone --branch v1.0 https://github.com/EVEShipFit/esf-format
cargo build --example program
uv run --project esf-format/tools esf-test -- target/debug/examples/program
cargo test
```
