//! Fits as type IDs, the way `load()` returns them and `save()` takes them (SPEC §5, §10).

use std::fmt;

/// One fit: a ship and everything on it, or a plain list of items.
///
/// A list of fits is one document. A fit that another fit carries, through
/// [`Entry::fit_name`], exists only there. Any other fit stands on its own.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EsfFit {
    /// The type ID of the ship, structure or container, like 587 for the Rifter.
    /// Only a type whose [`Kind::can_be_hull`](crate::Kind::can_be_hull) is true can be a hull.
    ///
    /// `None` for a fit without a ship, like the items in a contract: then every entry is
    /// stored in cargo, and the fit has no slots and no [`mode`](EsfFit::mode).
    pub hull: Option<u32>,
    /// The name of the fit, like "Shield Buffer". `None` for no name.
    pub name: Option<String>,
    /// The type ID of the tactical mode the ship is in, like the Hecate's Sharpshooter Mode.
    ///
    /// Only for a hull that has tactical modes; see [`Catalog::modes`](crate::Catalog::modes).
    /// `None` for none.
    pub mode: Option<u32>,
    /// The lines of the fit: items, empty slots and fits it carries.
    pub entries: Vec<Entry>,
}

/// One line of a fit: an item, an empty slot, or another fit carried by this one.
///
/// Only [`type_id`](Entry::type_id) is needed; every other field is `None` or empty when it
/// is the default, so start from [`Entry::default()`]. Entries can be in any order, except
/// within a rack: there, the order is the slot order, unless an entry sets
/// [`slot`](Entry::slot).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    /// The type ID of the item, like 2889 for a 200mm AutoCannon II.
    ///
    /// `None` for an empty slot. An empty slot must set [`location`](Entry::location) to its
    /// rack, like [`Location::High`].
    pub type_id: Option<u32>,
    /// How many there are. `None` means one.
    ///
    /// What a count means depends on where the item goes (esf/1 spec §6.2). For a fitted
    /// module or a deployed drone, it is that many of the same line: 3 guns take 3 slots.
    /// For a stored item, it is the size of one stack. For a fighter, it is how many are
    /// in the squadron; `None` means a full squadron. A plugged-in implant or booster takes
    /// no count.
    pub count: Option<u32>,
    /// The name of another fit in the same list, which this entry carries.
    ///
    /// Only for a type whose [`Kind::can_be_hull`](crate::Kind::can_be_hull) is true. The
    /// other fit must have this entry's [`type_id`](Entry::type_id) as its
    /// [`hull`](EsfFit::hull) and this as its [`name`](EsfFit::name), ignoring case. For
    /// example, a Heron with fit name "Scout" and [`Location::Frigate`] carries that fit in
    /// the Frigate Escape Bay. `None` means an unfitted ship or an empty container.
    pub fit_name: Option<String>,
    /// The type ID of the charge loaded into this item, like ammunition or a script. `None`
    /// for none.
    pub charge: Option<u32>,
    /// How many of [`charge`](Entry::charge) are loaded into each item.
    ///
    /// `None` means a full load: the item's capacity divided by the charge's volume, rounded
    /// down. Only set together with [`charge`](Entry::charge).
    pub charge_count: Option<u32>,
    /// The type ID of the mutaplasmid applied to this item, which makes it abyssal.
    ///
    /// [`type_id`](Entry::type_id) is then the base type the mutaplasmid applies to, and
    /// [`overrides`](Entry::overrides) holds the rolled values. `None` for a normal item.
    pub mutaplasmid: Option<u32>,
    /// Dogma attribute values that replace the type's own, as (attribute ID, value), like
    /// `(20, 524.0)` for speedFactor.
    ///
    /// Each value is the new base value, not a change on top of it. Attributes not listed
    /// keep the type's value. Empty for none.
    pub overrides: Vec<(u32, f64)>,
    /// Whether the item is offline, online or running. `None` means its default; see
    /// [`State`].
    pub state: Option<State>,
    /// Where the item goes. `None` means where its type goes by default; see [`Location`].
    pub location: Option<Location>,
    /// The slot in the [`location`](Entry::location) rack, counting from 1, like 3 for the
    /// third high slot or the third launch tube.
    ///
    /// Set [`location`](Entry::location) to the rack too, and the item must go in that rack.
    /// A pinned entry is one item, so leave [`count`](Entry::count) as `None`. A fighter
    /// squadron in a tube is the exception: there, the count is the squadron's size. Two
    /// entries cannot pin the same slot. `None` means the next free slot.
    /// [`load`](crate::load) never sets this: it gives the rack in slot order, with empty
    /// slots in between.
    pub slot: Option<u32>,
}

/// A whole document, as [`load_document`](crate::load_document) reads it: its fits, and the
/// data the library does not know.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    /// The fits, the same as [`load`](crate::load) returns.
    pub fits: Vec<EsfFit>,
    /// The blocks and binary fields that are not fits, in the order they are in the document.
    ///
    /// Empty when the document holds only fits.
    pub extra: Vec<Extra>,
}

/// Data in a document that is not a fit, like a fleet stored next to its fits.
///
/// To write it back, add it to what [`save`](crate::save) or
/// [`save_binary`](crate::save_binary) returns. For text, write the header line
/// `%name/version` and a line ending, then the text, and end the text with a line ending if
/// it has none. For binary, append [`binary_field`](crate::binary_field). Two documents put
/// one after the other are one document.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Extra {
    /// A block in a text document whose header is not `%esf/N`, like `%fleet/1`.
    Text {
        /// The name in the header, like "fleet".
        name: String,
        /// The version in the header, like 1. Leading zeros are dropped: `%fleet/01` gives 1.
        version: u32,
        /// Everything after the header line, up to the next header line or the end of the
        /// document, exactly as written. It keeps its line endings and blank lines, and can be
        /// empty.
        text: String,
    },
    /// A length-delimited field of a binary document, other than its fits (field 1).
    Binary {
        /// The field number.
        field: u32,
        /// The content of the field, without its key and length.
        bytes: Vec<u8>,
    },
}

/// Whether an item is offline, online or running, when that is not its default (esf/1 spec
/// §6.4).
///
/// By default, a fitted or deployed item runs when its type is
/// [`active`](crate::TypeRecord::active), and is online otherwise. An implant or booster is
/// online by default, and only takes [`Off`](State::Off). A stored item, in cargo, a bay or a
/// hold, has no state at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum State {
    /// Offline: fitted, but not using powergrid or CPU.
    Off = 1,
    /// Online, but not running, like a gun that is not shooting.
    On = 2,
    /// Overloaded, which also means running.
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

/// Where an entry goes, when not where its type goes by default (esf/1 spec §5).
///
/// An item goes where its [`Kind`](crate::Kind) says, so most entries leave this `None`. Set
/// it to store an item somewhere else, like a spare Damage Control II in
/// [`Cargo`](Location::Cargo).
/// So a drone with no location is out in space, and a fighter is in a launch tube. Set
/// [`Bay`](Location::Bay) to keep either one in its bay.
///
/// The racks, [`High`](Location::High) to [`Svc`](Location::Svc) and the launch tubes in
/// [`Tube`](Location::Tube), are for empty slots, and for an item put in a given
/// [`Entry::slot`]. Otherwise, an item never names its rack. The holds,
/// [`Ammo`](Location::Ammo) to [`Subsystem`](Location::Subsystem), only make sense on a hull
/// that has that hold, but the library does not check that.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Location {
    /// The high slot rack.
    High = 1,
    /// The mid slot rack.
    Mid,
    /// The low slot rack.
    Low,
    /// The rig rack.
    Rig,
    /// The subsystem rack. Not the same as [`Subsystem`](Location::Subsystem),
    /// which is a hold.
    Sub,
    /// The structure service rack.
    Svc,
    /// The cargo hold.
    Cargo,
    /// The drone bay or fighter bay, for drones and fighters that are not out in space or in a
    /// tube.
    Bay,
    /// The Ammo Hold.
    Ammo,
    /// The Booster Hold.
    Booster,
    /// The Command Center Hold.
    Command,
    /// The Corpse Hold.
    Corpse,
    /// The Mobile Depot Hold.
    Depot,
    /// The Expedition Hold.
    Expedition,
    /// The Fleet Hangar.
    Fleet,
    /// The Frigate Escape Bay.
    Frigate,
    /// The Fuel Bay.
    Fuel,
    /// The Gas Hold.
    Gas,
    /// The Ice Hold.
    Ice,
    /// The Infrastructure Hold.
    Infrastructure,
    /// The Ship Maintenance Bay.
    Maintenance,
    /// The Mineral Hold.
    Mineral,
    /// The Mining Hold.
    Mining,
    /// The Moon Material Output Bay.
    Moon,
    /// The Planetary Commodities Hold.
    Planetary,
    /// The Quafe Hold.
    Quafe,
    /// The Subsystem Hold, for spare subsystems. Not the same as [`Sub`](Location::Sub), which
    /// is the rack.
    Subsystem,
    /// The fighter launch tubes, a rack for fighters only.
    Tube,
}

impl Location {
    /// In binary order: the number of each is its index plus one.
    pub(crate) const ALL: [Location; 28] = [
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
        Location::Tube,
    ];

    /// The racks, in canonical order (§8).
    pub(crate) const RACKS: [Location; 7] = [
        Location::Sub,
        Location::High,
        Location::Mid,
        Location::Low,
        Location::Rig,
        Location::Svc,
        Location::Tube,
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
            Location::Tube => "tube",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<Location> {
        Location::ALL.into_iter().find(|l| l.as_str() == name)
    }

    /// Whether this is a rack of slots.
    pub fn is_rack(self) -> bool {
        self <= Location::Svc || self == Location::Tube
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
