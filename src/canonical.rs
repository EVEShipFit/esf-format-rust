//! Canonical form (SPEC §8).

use crate::catalog::{attribute_of, full_load};
use crate::names::{fold, shortest_unique};
use crate::number;
use crate::resolve::{Fit, Item, Place, Slot, default_place, kind_place, mode_name};
use crate::text::{SIGILS, is_count};
use crate::{
    AttributeRecord, Catalog, Entry, Error, EsfFit, Location, MutaplasmidRecord, State, TypeRecord,
};

pub(crate) fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

pub(crate) fn needs_quotes(name: &str) -> bool {
    let first = name.split(' ').next().unwrap_or_default();
    first.starts_with(['-', '%'])
        || is_count(first)
        || name
            .split(' ')
            .any(|word| word.as_bytes().first().is_none_or(|b| SIGILS.contains(b)))
}

pub(crate) fn quote_type(name: &str) -> String {
    if needs_quotes(name) {
        quote(name)
    } else {
        name.to_string()
    }
}

pub(crate) struct CanonLine<'a> {
    pub r#type: Option<&'a TypeRecord>,
    pub count: Option<u64>,
    pub fit_name: Option<&'a str>,
    pub charge: Option<&'a TypeRecord>,
    pub charge_count: Option<u32>,
    pub mutaplasmid: Option<&'a MutaplasmidRecord>,
    pub overrides: Vec<(&'a AttributeRecord, f64)>,
    pub state: Option<State>,
    pub location: Option<Location>,
    /// The line as text, without its count.
    pub body: String,
}

impl<'a> CanonLine<'a> {
    fn empty(rack: Location) -> CanonLine<'a> {
        CanonLine {
            r#type: None,
            count: None,
            fit_name: None,
            charge: None,
            charge_count: None,
            mutaplasmid: None,
            overrides: Vec::new(),
            state: None,
            location: Some(rack),
            body: format!("- @{rack}"),
        }
    }

    fn write(&self, out: &mut String) {
        if let Some(count) = self.count {
            out.push_str(&format!("{count}x "));
        }
        out.push_str(&self.body);
    }

    fn text(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn to_model(&self) -> Entry {
        Entry {
            type_id: self.r#type.map(|t| t.id),
            count: self
                .count
                .map(|count| count.try_into().expect("counts are checked")),
            fit_name: self.fit_name.map(str::to_string),
            charge: self.charge.map(|t| t.id),
            charge_count: self.charge_count,
            mutaplasmid: self.mutaplasmid.map(|m| m.mutaplasmid.id),
            overrides: self.overrides.iter().map(|(a, v)| (a.id, *v)).collect(),
            state: self.state,
            location: self.location,
        }
    }
}

pub(crate) struct CanonFit<'a> {
    pub hull: Option<&'a TypeRecord>,
    pub name: Option<&'a str>,
    pub mode: Option<&'a TypeRecord>,
    pub mode_text: Option<&'a str>,
    pub groups: Vec<Vec<CanonLine<'a>>>,
}

impl CanonFit<'_> {
    fn write(&self, out: &mut String) {
        out.push_str("%esf/1\n");
        out.push_str(
            &self
                .hull
                .map_or("-".to_string(), |hull| quote_type(&hull.name)),
        );
        if let Some(name) = self.name {
            out.push(' ');
            out.push_str(&quote(name));
        }
        if let Some(mode) = self.mode_text {
            out.push_str(" /");
            out.push_str(&quote_type(mode));
        }
        out.push('\n');
        for group in &self.groups {
            out.push('\n');
            for line in group {
                line.write(out);
                out.push('\n');
            }
        }
    }

    pub fn to_model(&self) -> EsfFit {
        EsfFit {
            hull: self.hull.map(|t| t.id),
            name: self.name.map(str::to_string),
            mode: self.mode.map(|t| t.id),
            entries: self
                .groups
                .iter()
                .flatten()
                .map(CanonLine::to_model)
                .collect(),
        }
    }
}

pub(crate) fn render(fits: &[CanonFit<'_>]) -> String {
    let mut out = String::new();
    for (at, fit) in fits.iter().enumerate() {
        if at > 0 {
            out.push('\n');
        }
        fit.write(&mut out);
    }
    out
}

/// The state an item has when its line names none; `None` is active.
fn default_state(item: &Item<'_>) -> Option<State> {
    (item.place.is_plugged() || !item.r#type.active).then_some(State::On)
}

/// Merge adjacent identical repeatable lines into one count.
fn collapse(lines: Vec<(bool, CanonLine<'_>)>) -> Result<Vec<CanonLine<'_>>, Error> {
    let mut out: Vec<(bool, CanonLine<'_>)> = Vec::with_capacity(lines.len());
    for (repeatable, line) in lines {
        if let Some((true, prev)) = out.last_mut()
            && repeatable
            && prev.body == line.body
        {
            let count = prev.count.unwrap_or(1) + line.count.unwrap_or(1);
            if count > u64::from(u32::MAX) {
                return Err(Error::new(format!(
                    "lines collapse into a count of {count}, which is too large"
                )));
            }
            prev.count = Some(count);
            continue;
        }
        out.push((repeatable, line));
    }
    Ok(out
        .into_iter()
        .map(|(_, mut line)| {
            line.count = line.count.filter(|&count| count != 1);
            line
        })
        .collect())
}

struct Context<'f, 'a> {
    fits: &'f [Fit<'a>],
    fit: &'f Fit<'a>,
    catalog: &'a dyn Catalog,
}

fn only<'i, 'a>(items: &[&'i Item<'a>], place: Place) -> Vec<&'i Item<'a>> {
    items
        .iter()
        .copied()
        .filter(|item| item.place == place)
        .collect()
}

impl<'a> Context<'_, 'a> {
    fn line(&self, item: &Item<'a>) -> Result<CanonLine<'a>, Error> {
        let source = item.line;
        let fit_name = item.reference.and_then(|at| self.fits[at].name());

        let charge_count = item.charge.and_then(|charge| {
            let full = full_load(item.r#type, charge);
            source
                .charge_count
                .filter(|&count| Some(u64::from(count)) != full)
        });

        let mut overrides: Vec<(&AttributeRecord, f64)> = item
            .overrides
            .iter()
            .map(|&(a, v)| (a, if v == 0.0 { 0.0 } else { v }))
            .collect();
        let mut mutaplasmid_name = None;
        if let Some(mutaplasmid) = item.mutaplasmid {
            let own = &mutaplasmid.mutaplasmid.name;
            let mut others: Vec<&str> = self
                .catalog
                .mutaplasmids(item.r#type.id)
                .iter()
                .map(|m| m.mutaplasmid.name.as_str())
                .collect();
            if let Some(at) = others.iter().position(|other| other == own) {
                others.remove(at);
            }
            mutaplasmid_name = Some(shortest_unique(own, &others));
            for &(attribute_id, base) in &mutaplasmid.rolls {
                if !overrides.iter().any(|(a, _)| a.id == attribute_id) {
                    overrides.push((attribute_of(self.catalog, attribute_id)?, base));
                }
            }
        }
        overrides.sort_by_cached_key(|(a, _)| (fold(&a.name), a.name.clone()));

        let state = source
            .state
            .filter(|&state| Some(state) != default_state(item));

        let location = match item.place {
            Place::At(location)
                if !location.is_rack()
                    && item.place != default_place(item.r#type.kind, self.fit.cargo_only) =>
            {
                Some(location)
            }
            _ => None,
        };

        let mut parts = vec![quote_type(&item.r#type.name)];
        if let Some(name) = fit_name {
            parts.push(quote(name));
        }
        if let Some(charge) = item.charge {
            let count = charge_count.map_or(String::new(), |count| format!("{count}x "));
            parts.push(format!(":{count}{}", quote_type(&charge.name)));
        }
        if let Some(name) = mutaplasmid_name {
            parts.push(format!("+{}", quote_type(name)));
        }
        if !overrides.is_empty() {
            let pairs: Vec<String> = overrides
                .iter()
                .map(|(a, v)| format!("{} {}", a.name, number::format(*v)))
                .collect();
            parts.push(format!("{{{}}}", pairs.join(", ")));
        }
        if let Some(state) = state {
            parts.push(format!("!{state}"));
        }
        if let Some(location) = location {
            parts.push(format!("@{location}"));
        }

        Ok(CanonLine {
            r#type: Some(item.r#type),
            count: None,
            fit_name,
            charge: item.charge,
            charge_count,
            mutaplasmid: item.mutaplasmid,
            overrides,
            state,
            location,
            body: parts.join(" "),
        })
    }

    fn rack(&self, rack: Location) -> Result<Vec<CanonLine<'a>>, Error> {
        let mut runs = self.fit.rack(rack);
        while let [rest @ .., last] = runs
            && last.item.is_none()
        {
            runs = rest;
        }
        let mut lines = Vec::with_capacity(runs.len());
        for run in runs {
            let mut line = match run.item.map(|at| &self.fit.slots[at]) {
                Some(Slot::Item(item)) => self.line(item)?,
                _ => CanonLine::empty(rack),
            };
            line.count = Some(run.length);
            lines.push((true, line));
        }
        collapse(lines)
    }

    fn sorted(&self, items: &[&Item<'a>]) -> Result<Vec<CanonLine<'a>>, Error> {
        let mut lines = Vec::with_capacity(items.len());
        for item in items {
            let mut line = self.line(item)?;
            let repeatable = !item.place.is_stored();
            let count = u64::from(item.line.count.unwrap_or(1));
            line.count = (repeatable || count != 1).then_some(count);
            let text = if repeatable {
                line.body.clone()
            } else {
                line.text()
            };
            lines.push(((fold(&item.r#type.name), text), repeatable, line));
        }
        lines.sort_by(|a, b| a.0.cmp(&b.0));
        collapse(
            lines
                .into_iter()
                .map(|(_, repeatable, line)| (repeatable, line))
                .collect(),
        )
    }

    fn fighters(&self, items: &[&Item<'a>]) -> Result<Vec<CanonLine<'a>>, Error> {
        let mut lines = Vec::new();
        for item in only(items, Place::Fighters) {
            let mut line = self.line(item)?;
            let count = item
                .line
                .count
                .filter(|&count| Some(count) != item.r#type.squadron_size);
            line.count = count.map(u64::from);
            lines.push(line);
        }
        lines.extend(self.sorted(&only(items, Place::At(Location::Bay)))?);
        Ok(lines)
    }

    fn drones(&self, items: &[&Item<'a>]) -> Result<Vec<CanonLine<'a>>, Error> {
        let mut lines = self.sorted(&only(items, Place::Drones))?;
        lines.extend(self.sorted(&only(items, Place::At(Location::Bay)))?);
        Ok(lines)
    }
}

const GROUPS: [Place; 30] = [
    Place::At(Location::Sub),
    Place::At(Location::High),
    Place::At(Location::Mid),
    Place::At(Location::Low),
    Place::At(Location::Rig),
    Place::At(Location::Svc),
    Place::Drones,
    Place::Fighters,
    Place::At(Location::Cargo),
    Place::At(Location::Ammo),
    Place::At(Location::Booster),
    Place::At(Location::Command),
    Place::At(Location::Corpse),
    Place::At(Location::Depot),
    Place::At(Location::Expedition),
    Place::At(Location::Fleet),
    Place::At(Location::Frigate),
    Place::At(Location::Fuel),
    Place::At(Location::Gas),
    Place::At(Location::Ice),
    Place::At(Location::Infrastructure),
    Place::At(Location::Maintenance),
    Place::At(Location::Mineral),
    Place::At(Location::Mining),
    Place::At(Location::Moon),
    Place::At(Location::Planetary),
    Place::At(Location::Quafe),
    Place::At(Location::Subsystem),
    Place::Implants,
    Place::Boosters,
];

pub(crate) fn canonical_fit<'a>(
    fits: &[Fit<'a>],
    fit: &Fit<'a>,
    catalog: &'a dyn Catalog,
) -> Result<CanonFit<'a>, Error> {
    let context = Context { fits, fit, catalog };
    let mut grouped: Vec<Vec<&Item<'a>>> = vec![Vec::new(); GROUPS.len()];
    for slot in &fit.slots {
        let Slot::Item(item) = slot else { continue };
        if item.place.is_rack() {
            continue;
        }
        let group = match item.place {
            Place::At(Location::Bay) => {
                kind_place(item.r#type.kind).expect("only drones and fighters go in the bay")
            }
            place => place,
        };
        let at = GROUPS
            .iter()
            .position(|&g| g == group)
            .expect("every place has a group");
        grouped[at].push(item);
    }

    let mut groups = Vec::new();
    for (group, items) in GROUPS.into_iter().zip(&grouped) {
        let lines = match group {
            Place::At(rack) if rack.is_rack() => context.rack(rack)?,
            Place::Drones => context.drones(items)?,
            Place::Fighters => context.fighters(items)?,
            _ => context.sorted(items)?,
        };
        if !lines.is_empty() {
            groups.push(lines);
        }
    }

    let mode_text = match (fit.hull, fit.mode) {
        (Some(hull), Some(mode)) => {
            let own = mode_name(hull, mode);
            let mut others: Vec<&str> = catalog
                .modes(hull.id)
                .iter()
                .map(|m| mode_name(hull, m))
                .collect();
            if let Some(at) = others.iter().position(|&other| other == own) {
                others.remove(at);
            }
            Some(shortest_unique(own, &others))
        }
        _ => None,
    };

    Ok(CanonFit {
        hull: fit.hull,
        name: fit.name(),
        mode: fit.mode,
        mode_text,
        groups,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert!(!needs_quotes("Damage Control II"));
        assert!(!needs_quotes("Ñ Damage Control"));
        assert!(needs_quotes(""));
        assert!(needs_quotes("-x"));
        assert!(needs_quotes("%x"));
        assert!(needs_quotes("3x Damage Control"));
        assert!(needs_quotes("Damage  Control"));
        assert!(needs_quotes("Damage @Control"));
        assert_eq!(
            quote_type("GDN-9 \"Nightstalker\""),
            "\"GDN-9 \"\"Nightstalker\"\"\""
        );
    }
}
