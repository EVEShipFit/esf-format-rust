//! The binary form (SPEC §10).

use std::collections::HashSet;

use crate::wire::{self, Reader, Value};
use crate::{Entry, Error, EsfFit, Location, State};

fn encode_entry(out: &mut Vec<u8>, entry: &Entry) {
    wire::uint(out, 1, entry.type_id);
    wire::uint(out, 2, entry.count);
    if let Some(name) = entry.fit_name.as_deref().filter(|n| !n.is_empty()) {
        wire::record(out, 3, name.as_bytes());
    }
    wire::uint(out, 4, entry.charge);
    wire::uint(out, 5, entry.charge_count);
    wire::uint(out, 6, entry.mutaplasmid);
    if !entry.overrides.is_empty() {
        let mut ids = Vec::new();
        let mut values = Vec::with_capacity(entry.overrides.len() * 8);
        for &(id, value) in &entry.overrides {
            wire::varint(&mut ids, u64::from(id));
            values.extend_from_slice(&value.to_le_bytes());
        }
        wire::record(out, 7, &ids);
        wire::record(out, 8, &values);
    }
    wire::uint(out, 9, entry.state.map(|s| s as u32));
    wire::uint(out, 10, entry.location.map(|l| l as u32));
}

fn encode_fit(out: &mut Vec<u8>, fit: &EsfFit) {
    wire::uint(out, 1, Some(1));
    wire::uint(out, 2, fit.hull);
    if let Some(name) = fit.name.as_deref().filter(|n| !n.is_empty()) {
        wire::record(out, 3, name.as_bytes());
    }
    wire::uint(out, 4, fit.mode);
    let mut payload = Vec::new();
    for entry in &fit.entries {
        payload.clear();
        encode_entry(&mut payload, entry);
        wire::record(out, 5, &payload);
    }
}

pub(crate) fn encode(fits: &[EsfFit]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut payload = Vec::new();
    for fit in fits {
        payload.clear();
        encode_fit(&mut payload, fit);
        wire::record(&mut out, 1, &payload);
    }
    out
}

fn uint32(value: u64, what: &str) -> Result<u32, Error> {
    u32::try_from(value).map_err(|_| Error::new(format!("binary: {what} is out of range")))
}

fn string(payload: &[u8], what: &str) -> Result<Option<String>, Error> {
    match std::str::from_utf8(payload) {
        Ok("") => Ok(None),
        Ok(text) => Ok(Some(text.to_string())),
        Err(_) => Err(Error::new(format!("binary: {what} is not valid UTF-8"))),
    }
}

fn state(value: u64) -> Result<Option<State>, Error> {
    match value {
        0 => Ok(None),
        _ => State::ALL
            .into_iter()
            .find(|&s| s as u64 == value)
            .map(Some)
            .ok_or_else(|| Error::new(format!("binary: unknown State value {value}"))),
    }
}

fn location(value: u64) -> Result<Option<Location>, Error> {
    match value {
        0 => Ok(None),
        _ => Location::ALL
            .into_iter()
            .find(|&l| l as u64 == value)
            .map(Some)
            .ok_or_else(|| Error::new(format!("binary: unknown Location value {value}"))),
    }
}

fn not_in_schema(what: &str) -> Error {
    Error::new(format!(
        "binary: {what} has a field that is not in the schema"
    ))
}

fn decode_entry(data: &[u8]) -> Result<Entry, Error> {
    let mut e = Entry::default();
    let mut attributes = Vec::new();
    let mut values = Vec::new();
    let mut reader = Reader::new(data);
    while !reader.done() {
        match reader.field()? {
            (1, Value::Varint(n)) => e.type_id = Some(uint32(n, "Entry.type")?),
            (2, Value::Varint(n)) => e.count = Some(uint32(n, "Entry.count")?),
            (3, Value::Len(b)) => e.fit_name = string(b, "Entry.fit_name")?,
            (4, Value::Varint(n)) => e.charge = Some(uint32(n, "Entry.charge")?),
            (5, Value::Varint(n)) => e.charge_count = Some(uint32(n, "Entry.charge_count")?),
            (6, Value::Varint(n)) => e.mutaplasmid = Some(uint32(n, "Entry.mutaplasmid")?),
            (7, Value::Varint(n)) => attributes.push(uint32(n, "Entry.override_attributes")?),
            (7, Value::Len(b)) => {
                for n in wire::unpack_varints(b)? {
                    attributes.push(uint32(n, "Entry.override_attributes")?);
                }
            }
            (8, Value::I64(b)) => values.push(f64::from_le_bytes(b)),
            (8, Value::Len(b)) => values.extend(wire::unpack_doubles(b)?),
            (9, Value::Varint(n)) => e.state = state(n)?,
            (10, Value::Varint(n)) => e.location = location(n)?,
            _ => return Err(not_in_schema("Entry")),
        }
    }
    if attributes.len() != values.len() {
        return Err(Error::new("binary: override fields have different lengths"));
    }
    let mut seen = HashSet::with_capacity(attributes.len());
    if !attributes.iter().all(|&id| seen.insert(id)) {
        return Err(Error::new("binary: an attribute is overridden twice"));
    }
    e.overrides = attributes.into_iter().zip(values).collect();
    Ok(e)
}

fn decode_fit(data: &[u8]) -> Result<EsfFit, Error> {
    let mut fit = EsfFit::default();
    let mut version = 0;
    let mut reader = Reader::new(data);
    while !reader.done() {
        match reader.field()? {
            (1, Value::Varint(n)) => version = uint32(n, "Fit.version")?,
            (2, Value::Varint(n)) => fit.hull = Some(uint32(n, "Fit.hull")?),
            (3, Value::Len(b)) => fit.name = string(b, "Fit.name")?,
            (4, Value::Varint(n)) => fit.mode = Some(uint32(n, "Fit.tactical_mode")?),
            (5, Value::Len(b)) => fit.entries.push(decode_entry(b)?),
            _ => return Err(not_in_schema("Fit")),
        }
    }
    if version != 1 {
        return Err(Error::new(format!(
            "binary: unsupported Fit.version {version}"
        )));
    }
    Ok(fit)
}

pub(crate) fn decode(data: &[u8]) -> Result<Vec<EsfFit>, Error> {
    let mut fits = Vec::new();
    let mut reader = Reader::new(data);
    while !reader.done() {
        match reader.field()? {
            (1, Value::Len(b)) => fits.push(decode_fit(b)?),
            (1, _) => return Err(Error::new("binary: Document.fits has the wrong wire type")),
            _ => {}
        }
    }
    if fits.is_empty() {
        return Err(Error::new("binary: a document holds at least one fit"));
    }
    Ok(fits)
}
