//! Fits as the text they stand for, and the check that their IDs match it (SPEC §10.2).

use crate::canonical::{quote, quote_type, type_ref};
use crate::number;
use crate::resolve::{Fit, Slot};
use crate::{Catalog, Entry, Error, EsfFit, Location, TypeRecord};

/// The (fit, entry) each line of the source text is, counting from 1.
pub(crate) type Where = Vec<(usize, Option<usize>)>;

fn name(text: &str) -> Result<String, Error> {
    if text.contains(['\r', '\n']) {
        return Err(Error::format("a fit name contains CR or LF"));
    }
    Ok(quote(text))
}

fn type_of<'a>(
    catalog: &'a dyn Catalog,
    type_id: u32,
    what: &str,
) -> Result<&'a TypeRecord, Error> {
    match catalog.type_by_id(type_id) {
        Some(t) if t.name.contains(['\r', '\n']) => Err(Error::format(format!(
            "the name of {what} type ID {type_id} contains CR or LF"
        ))),
        Some(t) => Ok(t),
        None => Err(Error::catalog(format!("unknown {what} type ID {type_id}"))),
    }
}

fn type_name(catalog: &dyn Catalog, type_id: u32, what: &str) -> Result<String, Error> {
    Ok(quote_type(&type_of(catalog, type_id, what)?.name))
}

fn type_id_or_name(catalog: &dyn Catalog, type_id: u32, what: &str) -> Result<String, Error> {
    Ok(type_ref(catalog, type_of(catalog, type_id, what)?))
}

fn entry(e: &Entry, catalog: &dyn Catalog) -> Result<String, Error> {
    if e.count == Some(0) || e.charge_count == Some(0) {
        return Err(Error::rule("a count must be at least 1"));
    }
    if e.slot == Some(0) {
        return Err(Error::rule("a slot must be at least 1"));
    }
    if e.slot.is_some() && !e.location.is_some_and(Location::is_rack) {
        return Err(Error::rule("a slot needs the rack it is in"));
    }
    if e.type_id.is_some() && e.slot.is_none() && e.location.is_some_and(Location::is_rack) {
        return Err(Error::rule("an item names a rack only to pin a slot"));
    }
    if e.charge_count.is_some() && e.charge.is_none() {
        return Err(Error::rule("a charge count is set without a charge"));
    }

    let mut parts = Vec::new();
    if let Some(count) = e.count {
        parts.push(format!("{count}x"));
    }
    parts.push(match e.type_id {
        Some(type_id) => type_id_or_name(catalog, type_id, "item")?,
        None => "-".to_string(),
    });
    if let Some(fit_name) = e.fit_name.as_deref().filter(|n| !n.is_empty()) {
        parts.push(name(fit_name)?);
    }
    if let Some(charge) = e.charge {
        let count = e
            .charge_count
            .map_or(String::new(), |count| format!("{count}x "));
        parts.push(format!(
            ":{count}{}",
            type_id_or_name(catalog, charge, "charge")?
        ));
    }
    if let Some(mutaplasmid) = e.mutaplasmid {
        parts.push(format!(
            "+{}",
            type_name(catalog, mutaplasmid, "mutaplasmid")?
        ));
    }
    if !e.overrides.is_empty() {
        let mut pairs = Vec::with_capacity(e.overrides.len());
        for &(attribute_id, value) in &e.overrides {
            let Some(attribute) = catalog.attribute_by_id(attribute_id) else {
                return Err(Error::catalog(format!(
                    "unknown attribute ID {attribute_id}"
                )));
            };
            if !value.is_finite() {
                return Err(Error::rule("an override value is not finite"));
            }
            pairs.push(format!("{} {}", attribute.name, number::format(value)));
        }
        parts.push(format!("{{{}}}", pairs.join(", ")));
    }
    if let Some(state) = e.state {
        parts.push(format!("!{state}"));
    }
    if let Some(location) = e.location {
        let slot = e.slot.map_or(String::new(), |slot| slot.to_string());
        parts.push(format!("@{location}{slot}"));
    }
    Ok(parts.join(" "))
}

fn hull(fit: &EsfFit, catalog: &dyn Catalog) -> Result<String, Error> {
    let mut parts = vec![match fit.hull {
        Some(hull) => type_id_or_name(catalog, hull, "hull")?,
        None => "-".to_string(),
    }];
    if let Some(fit_name) = fit.name.as_deref().filter(|n| !n.is_empty()) {
        parts.push(name(fit_name)?);
    }
    if let Some(mode) = fit.mode {
        parts.push(format!("/{}", type_name(catalog, mode, "tactical mode")?));
    }
    Ok(parts.join(" "))
}

/// The text document the fits stand for, and where each line came from.
pub(crate) fn source(fits: &[EsfFit], catalog: &dyn Catalog) -> Result<(String, Where), Error> {
    if fits.is_empty() {
        return Err(Error::format("a document holds at least one fit"));
    }
    let mut out = String::new();
    let mut lines = Where::new();
    for (fit, n) in fits.iter().zip(1..) {
        let hull = hull(fit, catalog).map_err(|e| e.in_fit(n, None))?;
        out.push_str("%esf/1\n");
        out.push_str(&hull);
        out.push('\n');
        lines.extend([(n, None), (n, None)]);
        for (e, m) in fit.entries.iter().zip(1..) {
            out.push_str(&entry(e, catalog).map_err(|e| e.in_fit(n, Some(m)))?);
            out.push('\n');
            lines.push((n, Some(m)));
        }
    }
    Ok((out, lines))
}

fn same(got: Option<u32>, want: Option<u32>, what: &str) -> Result<(), Error> {
    if got == want {
        return Ok(());
    }
    Err(Error::rule(format!(
        "{what} ID {} is not what its name resolves to ({})",
        show(want),
        show(got)
    )))
}

fn show(id: Option<u32>) -> String {
    id.map_or("None".to_string(), |id| id.to_string())
}

/// Check that every ID is the one its name resolves to.
pub(crate) fn verify(resolved: &[Fit<'_>], fits: &[EsfFit]) -> Result<(), Error> {
    for ((fit, given), n) in resolved.iter().zip(fits).zip(1..) {
        same(fit.hull.map(|t| t.id), given.hull, "hull").map_err(|e| e.in_fit(n, None))?;
        same(fit.mode.map(|t| t.id), given.mode, "tactical mode").map_err(|e| e.in_fit(n, None))?;
        for ((slot, entry), m) in fit.slots.iter().zip(&given.entries).zip(1..) {
            let Slot::Item(item) = slot else { continue };
            let check = || {
                same(Some(item.r#type.id), entry.type_id, "type")?;
                same(item.charge.map(|t| t.id), entry.charge, "charge")?;
                same(
                    item.mutaplasmid.map(|m| m.mutaplasmid.id),
                    entry.mutaplasmid,
                    "mutaplasmid",
                )?;
                let got: Vec<u32> = item.overrides.iter().map(|(a, _)| a.id).collect();
                let want: Vec<u32> = entry.overrides.iter().map(|&(id, _)| id).collect();
                if got != want {
                    return Err(Error::rule(format!(
                        "attribute IDs {want:?} resolve to {got:?}"
                    )));
                }
                Ok(())
            };
            check().map_err(|e| e.in_fit(n, Some(m)))?;
        }
    }
    Ok(())
}
