//! Resolving names against the Catalog, and the semantic rules (SPEC §4.2, §5, §6).

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::names::{fold, is_word_prefix};
use crate::text::{EmptyLine, FitBlock, ItemLine, Line};
use crate::{
    AttributeRecord, Catalog, Error, Kind, Location, MutaplasmidRecord, State, TypeRecord,
};

/// Where an item is: a Location, or a place only its type can put it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Place {
    At(Location),
    Drones,
    Fighters,
    Implants,
    Boosters,
}

impl Place {
    pub fn is_stored(self) -> bool {
        matches!(self, Place::At(location) if location.is_stored())
    }

    pub fn is_rack(self) -> bool {
        matches!(self, Place::At(location) if location.is_rack())
    }

    pub fn is_plugged(self) -> bool {
        matches!(self, Place::Implants | Place::Boosters)
    }
}

/// The place a kind of type goes, in a fit with a ship.
pub(crate) fn kind_place(kind: Kind) -> Option<Place> {
    Some(match kind {
        Kind::Sub => Place::At(Location::Sub),
        Kind::High => Place::At(Location::High),
        Kind::Mid => Place::At(Location::Mid),
        Kind::Low => Place::At(Location::Low),
        Kind::Rig => Place::At(Location::Rig),
        Kind::Svc => Place::At(Location::Svc),
        Kind::Drone => Place::Drones,
        Kind::Fighter => Place::Fighters,
        Kind::Implant => Place::Implants,
        Kind::Booster => Place::Boosters,
        _ => return None,
    })
}

pub(crate) fn default_place(kind: Kind, cargo_only: bool) -> Place {
    if cargo_only {
        Place::At(Location::Cargo)
    } else {
        kind_place(kind).unwrap_or(Place::At(Location::Cargo))
    }
}

pub(crate) struct Item<'a> {
    pub line: &'a ItemLine<'a>,
    pub r#type: &'a TypeRecord,
    pub place: Place,
    pub charge: Option<&'a TypeRecord>,
    pub mutaplasmid: Option<&'a MutaplasmidRecord>,
    pub overrides: Vec<(&'a AttributeRecord, f64)>,
    /// The fit this item carries, by its index in the document.
    pub reference: Option<usize>,
}

pub(crate) enum Slot<'a> {
    Empty(&'a EmptyLine),
    Item(Item<'a>),
}

impl Slot<'_> {
    fn place(&self) -> Place {
        match self {
            Slot::Empty(line) => Place::At(line.rack),
            Slot::Item(item) => item.place,
        }
    }
}

/// Consecutive slots of a rack holding the same item, by its index in the fit's slots.
pub(crate) struct Run {
    pub start: u64,
    pub length: u64,
    pub item: Option<usize>,
}

pub(crate) struct Fit<'a> {
    pub block: &'a FitBlock<'a>,
    pub hull: Option<&'a TypeRecord>,
    pub mode: Option<&'a TypeRecord>,
    pub cargo_only: bool,
    pub slots: Vec<Slot<'a>>,
    pub racks: Vec<(Location, Vec<Run>)>,
}

impl<'a> Fit<'a> {
    pub fn name(&self) -> Option<&'a str> {
        self.block.fit_name.as_deref()
    }

    pub fn rack(&self, rack: Location) -> &[Run] {
        self.racks
            .iter()
            .find(|(location, _)| *location == rack)
            .map_or(&[], |(_, runs)| runs)
    }
}

/// A tactical mode's name without the hull's name.
pub(crate) fn mode_name<'a>(hull: &TypeRecord, mode: &'a TypeRecord) -> &'a str {
    let skip = hull.name.chars().count();
    let rest = mode
        .name
        .char_indices()
        .nth(skip)
        .map_or("", |(at, _)| &mode.name[at..]);
    rest.trim_start_matches(' ')
}

fn type_named<'a>(
    catalog: &'a dyn Catalog,
    name: &str,
    line: usize,
) -> Result<&'a TypeRecord, Error> {
    catalog
        .type_by_name(&fold(name))
        .ok_or_else(|| Error::at(line, format!("unknown type {name:?}")))
}

fn pick<'a, T>(
    candidates: Vec<&'a T>,
    given: &str,
    what: &str,
    line: usize,
    name: impl Fn(&T) -> &str,
) -> Result<&'a T, Error> {
    match candidates[..] {
        [] => Err(Error::at(line, format!("no {what} matches {given:?}"))),
        [one] => Ok(one),
        _ => {
            let names: Vec<String> = candidates
                .iter()
                .map(|c| format!("{:?}", name(c)))
                .collect();
            let names = names.join(", ");
            Err(Error::at(
                line,
                format!("{given:?} matches more than one {what}: {names}"),
            ))
        }
    }
}

fn resolve_hull<'a>(block: &'a FitBlock<'a>, catalog: &'a dyn Catalog) -> Result<Fit<'a>, Error> {
    let mut fit = Fit {
        block,
        hull: None,
        mode: None,
        cargo_only: true,
        slots: Vec::new(),
        racks: Vec::new(),
    };
    let Some(name) = &block.hull else {
        return Ok(fit);
    };

    let line = block.hull_line;
    let hull = type_named(catalog, name, line)?;
    if !matches!(hull.kind, Kind::Hull | Kind::Container) {
        return Err(Error::at(
            line,
            format!("{:?} is not a ship, structure or container", hull.name),
        ));
    }
    fit.hull = Some(hull);
    fit.cargo_only = hull.kind == Kind::Container;

    if let Some(given) = &block.mode {
        let modes = catalog.modes(hull.id);
        if modes.is_empty() {
            return Err(Error::at(
                line,
                format!("{:?} has no tactical modes", hull.name),
            ));
        }
        let matches = modes
            .iter()
            .filter(|m| is_word_prefix(given, &m.name) || is_word_prefix(given, mode_name(hull, m)))
            .collect();
        let what = format!("tactical mode of {:?}", hull.name);
        fit.mode = Some(pick(matches, given, &what, line, |m| &m.name)?);
    }
    Ok(fit)
}

type Named = HashMap<(u32, String), Vec<usize>>;

fn resolve_item<'a>(
    line: &'a ItemLine<'a>,
    fit: &Fit<'a>,
    named: &Named,
    catalog: &'a dyn Catalog,
) -> Result<Item<'a>, Error> {
    let n = line.number;
    let t = type_named(catalog, &line.name, n)?;
    if t.kind == Kind::Mode {
        return Err(Error::at(
            n,
            "a tactical mode is only written on the hull line",
        ));
    }

    let mut item = Item {
        line,
        r#type: t,
        place: default_place(t.kind, fit.cargo_only),
        charge: None,
        mutaplasmid: None,
        overrides: Vec::new(),
        reference: None,
    };

    if let Some(fit_name) = &line.fit_name {
        if !matches!(t.kind, Kind::Hull | Kind::Container) {
            return Err(Error::at(n, format!("{:?} cannot take a fit name", t.name)));
        }
        match named.get(&(t.id, fold(fit_name))).map(Vec::as_slice) {
            Some(&[one]) => item.reference = Some(one),
            found => {
                let what = if found.is_none() {
                    "no fit"
                } else {
                    "more than one fit"
                };
                return Err(Error::at(
                    n,
                    format!("{what} matches {} {fit_name:?}", t.name),
                ));
            }
        }
    }

    if let Some(location) = line.location {
        if fit.cargo_only && location != Location::Cargo {
            return Err(Error::at(
                n,
                "a fit without a ship stores everything in cargo",
            ));
        }
        if line.index.is_some() && kind_place(t.kind) != Some(Place::At(location)) {
            return Err(Error::at(
                n,
                format!("{:?} does not go in the {location} rack", t.name),
            ));
        }
        if location == Location::Bay && !matches!(t.kind, Kind::Drone | Kind::Fighter) {
            return Err(Error::at(n, "@bay is only for drones and fighters"));
        }
        item.place = Place::At(location);
    }

    if item.place.is_plugged() && line.count.is_some() {
        return Err(Error::at(
            n,
            "a plugged-in implant or booster takes no count",
        ));
    }

    if let Some(state) = line.state {
        if item.place.is_stored() {
            return Err(Error::at(n, "a stored item has no state"));
        }
        if item.place.is_plugged() && state != State::Off {
            return Err(Error::at(n, "an implant or booster takes !off only"));
        }
    }

    if let Some(charge) = &line.charge {
        let charge = type_named(catalog, charge, n)?;
        if charge.kind != Kind::Charge {
            return Err(Error::at(n, format!("{:?} is not a charge", charge.name)));
        }
        item.charge = Some(charge);
    }

    if let Some(given) = &line.mutaplasmid {
        let matches = catalog
            .mutaplasmids(t.id)
            .iter()
            .filter(|m| is_word_prefix(given, &m.mutaplasmid.name))
            .collect();
        let what = format!("mutaplasmid for {:?}", t.name);
        item.mutaplasmid = Some(pick(matches, given, &what, n, |m| &m.mutaplasmid.name)?);
    }

    let mut seen = HashSet::new();
    for &(name, value) in &line.overrides {
        let Some(attribute) = catalog.attribute_by_name(&fold(name)) else {
            return Err(Error::at(n, format!("unknown attribute {name:?}")));
        };
        if !seen.insert(attribute.id) {
            return Err(Error::at(
                n,
                format!("attribute {:?} is overridden twice", attribute.name),
            ));
        }
        let number: f64 = value.parse().expect("the parser only passes decimals");
        if !number.is_finite() {
            return Err(Error::at(n, format!("value {value:?} is out of range")));
        }
        item.overrides.push((attribute, number));
    }

    Ok(item)
}

fn resolve_slot<'a>(
    line: &'a Line<'a>,
    fit: &Fit<'a>,
    named: &Named,
    catalog: &'a dyn Catalog,
) -> Result<Slot<'a>, Error> {
    match line {
        Line::Empty(line) if fit.cargo_only => Err(Error::at(
            line.number,
            "a fit without a ship has no slots to keep empty",
        )),
        Line::Empty(line) => Ok(Slot::Empty(line)),
        Line::Item(line) => resolve_item(line, fit, named, catalog).map(Slot::Item),
    }
}

fn references<'f>(fit: &'f Fit<'_>) -> impl Iterator<Item = (&'f Item<'f>, usize)> + 'f {
    fit.slots.iter().filter_map(|slot| match slot {
        Slot::Item(item) => item.reference.map(|reference| (item, reference)),
        Slot::Empty(_) => None,
    })
}

/// A fit cannot contain itself, directly or through other fits.
fn check_cycles(fits: &[Fit<'_>]) -> Result<(), Error> {
    let mut done = vec![false; fits.len()];
    let mut on_path = vec![false; fits.len()];
    for start in 0..fits.len() {
        if done[start] {
            continue;
        }
        on_path[start] = true;
        let mut stack = vec![(start, references(&fits[start]))];
        while let Some((fit, items)) = stack.last_mut() {
            let fit = *fit;
            let Some((item, reference)) = items.next() else {
                stack.pop();
                on_path[fit] = false;
                done[fit] = true;
                continue;
            };
            if on_path[reference] {
                return Err(Error::at(item.line.number, "a fit cannot contain itself"));
            }
            if !done[reference] {
                on_path[reference] = true;
                stack.push((reference, references(&fits[reference])));
            }
        }
    }
    Ok(())
}

fn line_of<'a>(slot: &'a Slot<'a>) -> (usize, Option<u32>, Option<u32>) {
    match slot {
        Slot::Empty(line) => (line.number, line.count, line.index),
        Slot::Item(item) => (item.line.number, item.line.count, item.line.index),
    }
}

fn filled(slots: &[Slot<'_>], at: usize) -> Option<usize> {
    matches!(slots[at], Slot::Item(_)).then_some(at)
}

/// Assign slots in one rack (§6.3), given the indices of its slots.
fn layout(slots: &[Slot<'_>], members: &[usize]) -> Result<Vec<Run>, Error> {
    let mut pinned = BTreeMap::new();
    for &at in members {
        let (number, _, index) = line_of(&slots[at]);
        let Some(index) = index else { continue };
        if pinned.insert(u64::from(index), at).is_some() {
            return Err(Error::at(number, format!("slot {index} is pinned twice")));
        }
    }

    let mut runs: Vec<Run> = pinned
        .iter()
        .map(|(&start, &at)| Run {
            start,
            length: 1,
            item: filled(slots, at),
        })
        .collect();
    let mut pos = 1;
    for &at in members {
        let (_, count, index) = line_of(&slots[at]);
        if index.is_some() {
            continue;
        }
        let mut remaining = u64::from(count.unwrap_or(1));
        while remaining > 0 {
            while pinned.contains_key(&pos) {
                pos += 1;
            }
            let room = pinned
                .range(pos..)
                .next()
                .map_or(remaining, |(&pin, _)| pin - pos);
            let take = remaining.min(room);
            runs.push(Run {
                start: pos,
                length: take,
                item: filled(slots, at),
            });
            pos += take;
            remaining -= take;
        }
    }
    runs.sort_by_key(|run| run.start);

    let mut gapless = Vec::with_capacity(runs.len());
    let mut pos = 1;
    for run in runs {
        if run.start > pos {
            gapless.push(Run {
                start: pos,
                length: run.start - pos,
                item: None,
            });
        }
        pos = run.start + run.length;
        gapless.push(run);
    }
    Ok(gapless)
}

fn check_plugged(slots: &[Slot<'_>]) -> Result<(), Error> {
    let mut plugged = HashSet::new();
    for slot in slots {
        let Slot::Item(item) = slot else { continue };
        if item.place.is_plugged() && !plugged.insert(item.r#type.id) {
            let message = format!("{:?} is plugged in twice", item.r#type.name);
            return Err(Error::at(item.line.number, message));
        }
    }
    Ok(())
}

pub(crate) fn resolve<'a>(
    blocks: &'a [FitBlock<'a>],
    catalog: &'a dyn Catalog,
) -> Result<Vec<Fit<'a>>, Error> {
    let mut fits = blocks
        .iter()
        .map(|block| resolve_hull(block, catalog))
        .collect::<Result<Vec<_>, _>>()?;

    let mut named = Named::new();
    for (at, fit) in fits.iter().enumerate() {
        if let (Some(hull), Some(name)) = (fit.hull, fit.name()) {
            named.entry((hull.id, fold(name))).or_default().push(at);
        }
    }

    let mut resolved = Vec::with_capacity(fits.len());
    for fit in &fits {
        let slots = fit
            .block
            .lines
            .iter()
            .map(|line| resolve_slot(line, fit, &named, catalog))
            .collect::<Result<Vec<_>, _>>()?;
        check_plugged(&slots)?;
        resolved.push(slots);
    }
    for (fit, slots) in fits.iter_mut().zip(resolved) {
        fit.slots = slots;
    }

    check_cycles(&fits)?;

    for fit in &mut fits {
        for rack in Location::RACKS {
            let members: Vec<usize> = (0..fit.slots.len())
                .filter(|&at| fit.slots[at].place() == Place::At(rack))
                .collect();
            if !members.is_empty() {
                let runs = layout(&fit.slots, &members)?;
                fit.racks.push((rack, runs));
            }
        }
    }
    Ok(fits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(name: &str) -> TypeRecord {
        TypeRecord {
            id: 1,
            name: name.to_string(),
            kind: Kind::Mode,
            active: false,
            capacity: 0.0,
            volume: 0.0,
            squadron_size: None,
        }
    }

    #[test]
    fn mode_names() {
        let hull = record("Hecate");
        assert_eq!(
            mode_name(&hull, &record("Hecate Defense Mode")),
            "Defense Mode"
        );
        assert_eq!(mode_name(&hull, &record("Mode")), "");
        assert_eq!(mode_name(&record("Ñ"), &record("Ñé Mode")), "é Mode");
    }
}
