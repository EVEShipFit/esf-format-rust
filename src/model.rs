//! Fits as type IDs, the way `load()` returns them and `save()` takes them (SPEC §5, §10).

use std::fmt;

/// A fit: its hull and its entries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EsfFit {
    /// The ship, structure or container; `None` for a fit without a ship.
    pub hull: Option<u32>,
    /// The fit name.
    pub name: Option<String>,
    /// The tactical mode.
    pub mode: Option<u32>,
    /// One per line of the fit.
    pub entries: Vec<Entry>,
}

/// One line of a fit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    /// The type; `None` for an empty slot.
    pub type_id: Option<u32>,
    /// How many.
    pub count: Option<u32>,
    /// The name of another fit in the list, which this ship or container carries.
    pub fit_name: Option<String>,
    /// The charge loaded.
    pub charge: Option<u32>,
    /// How many charges are loaded.
    pub charge_count: Option<u32>,
    /// The mutaplasmid applied.
    pub mutaplasmid: Option<u32>,
    /// Attribute values by attribute ID.
    pub overrides: Vec<(u32, f64)>,
    /// The state, when not its default.
    pub state: Option<State>,
    /// Where the entry is, when not where its type goes by default.
    pub location: Option<Location>,
}

/// The state of an item, when not its default (§6.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum State {
    Off = 1,
    On = 2,
    Heat = 3,
}

impl State {
    pub(crate) const ALL: [State; 3] = [State::Off, State::On, State::Heat];

    /// The name as written after `!`.
    pub fn as_str(self) -> &'static str {
        match self {
            State::Off => "off",
            State::On => "on",
            State::Heat => "heat",
        }
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where an entry is, when not where its type goes by default (§5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Location {
    High = 1,
    Mid,
    Low,
    Rig,
    Sub,
    Svc,
    Cargo,
    Bay,
    Ammo,
    Booster,
    Command,
    Corpse,
    Depot,
    Expedition,
    Fleet,
    Frigate,
    Fuel,
    Gas,
    Ice,
    Infrastructure,
    Maintenance,
    Mineral,
    Mining,
    Moon,
    Planetary,
    Quafe,
    Subsystem,
}

impl Location {
    /// In binary order: the number of each is its index plus one.
    pub(crate) const ALL: [Location; 27] = [
        Location::High,
        Location::Mid,
        Location::Low,
        Location::Rig,
        Location::Sub,
        Location::Svc,
        Location::Cargo,
        Location::Bay,
        Location::Ammo,
        Location::Booster,
        Location::Command,
        Location::Corpse,
        Location::Depot,
        Location::Expedition,
        Location::Fleet,
        Location::Frigate,
        Location::Fuel,
        Location::Gas,
        Location::Ice,
        Location::Infrastructure,
        Location::Maintenance,
        Location::Mineral,
        Location::Mining,
        Location::Moon,
        Location::Planetary,
        Location::Quafe,
        Location::Subsystem,
    ];

    /// The racks, in canonical order (§8).
    pub(crate) const RACKS: [Location; 6] = [
        Location::Sub,
        Location::High,
        Location::Mid,
        Location::Low,
        Location::Rig,
        Location::Svc,
    ];

    /// The name as written after `@`.
    pub fn as_str(self) -> &'static str {
        match self {
            Location::High => "high",
            Location::Mid => "mid",
            Location::Low => "low",
            Location::Rig => "rig",
            Location::Sub => "sub",
            Location::Svc => "svc",
            Location::Cargo => "cargo",
            Location::Bay => "bay",
            Location::Ammo => "ammo",
            Location::Booster => "booster",
            Location::Command => "command",
            Location::Corpse => "corpse",
            Location::Depot => "depot",
            Location::Expedition => "expedition",
            Location::Fleet => "fleet",
            Location::Frigate => "frigate",
            Location::Fuel => "fuel",
            Location::Gas => "gas",
            Location::Ice => "ice",
            Location::Infrastructure => "infrastructure",
            Location::Maintenance => "maintenance",
            Location::Mineral => "mineral",
            Location::Mining => "mining",
            Location::Moon => "moon",
            Location::Planetary => "planetary",
            Location::Quafe => "quafe",
            Location::Subsystem => "subsystem",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<Location> {
        Location::ALL.into_iter().find(|l| l.as_str() == name)
    }

    /// Whether this is a rack of slots.
    pub fn is_rack(self) -> bool {
        self <= Location::Svc
    }

    /// Whether this is the cargo, the bay or another hold.
    pub fn is_stored(self) -> bool {
        !self.is_rack()
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
