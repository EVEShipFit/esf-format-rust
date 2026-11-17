//! Helpers to build a [`Catalog`](crate::Catalog) from the SDE, for the fields that take more
//! than a lookup.
//!
//! Each helper takes plain values from the SDE files, so it works however you load them.

use std::collections::HashMap;

use crate::{Kind, fold};

const CATEGORY_SHIP: u32 = 6;
const CATEGORY_MODULE: u32 = 7;
const CATEGORY_CHARGE: u32 = 8;
const CATEGORY_DRONE: u32 = 18;
const CATEGORY_IMPLANT: u32 = 20;
const CATEGORY_SUBSYSTEM: u32 = 32;
const CATEGORY_STRUCTURE: u32 = 65;
const CATEGORY_STRUCTURE_MODULE: u32 = 66;
const CATEGORY_FIGHTER: u32 = 87;

const GROUP_BOOSTER: u32 = 303;
const GROUP_SHIP_MODIFIERS: u32 = 1306;
const GROUP_CONTAINERS: [u32; 4] = [12, 340, 448, 649];

const EFFECT_ONLINE: u32 = 16;
const EFFECT_CATEGORY_ACTIVE: u32 = 1;
const EFFECT_CATEGORY_TARGET: u32 = 2;
const SLOT_EFFECTS: [(u32, Kind); 5] = [
    (12, Kind::High),
    (13, Kind::Mid),
    (11, Kind::Low),
    (2663, Kind::Rig),
    (6306, Kind::Svc),
];

const ATTRIBUTE_SQUADRON_SIZE: u32 = 2215;

/// Works out what a type is. See [`Kind`] for each rule.
///
/// `category_id` is the `categoryID` of the type's group, in `groups.jsonl`.
/// `group_id` is the type's `groupID`, in `types.jsonl`.
/// `effect_ids` are the `effectID`s in the type's `dogmaEffects`, in `typeDogma.jsonl`.
/// The first slot effect in `effect_ids` wins, so pass them in the order of the file.
///
/// ```
/// use esf::Kind;
///
/// // 200mm AutoCannon II: category Module, group Projectile Weapon, effect hiPower among others.
/// assert_eq!(esf::sde::kind(7, 55, [16, 12, 34]), Kind::High);
/// ```
pub fn kind(category_id: u32, group_id: u32, effect_ids: impl IntoIterator<Item = u32>) -> Kind {
    if group_id == GROUP_SHIP_MODIFIERS {
        return Kind::Mode;
    }
    if GROUP_CONTAINERS.contains(&group_id) {
        return Kind::Container;
    }
    if group_id == GROUP_BOOSTER {
        return Kind::Booster;
    }
    match category_id {
        CATEGORY_SHIP | CATEGORY_STRUCTURE => Kind::Hull,
        CATEGORY_MODULE | CATEGORY_STRUCTURE_MODULE => effect_ids
            .into_iter()
            .find_map(|effect_id| {
                SLOT_EFFECTS
                    .iter()
                    .find(|&&(slot, _)| slot == effect_id)
                    .map(|&(_, kind)| kind)
            })
            .unwrap_or(Kind::Other),
        CATEGORY_SUBSYSTEM => Kind::Sub,
        CATEGORY_DRONE => Kind::Drone,
        CATEGORY_FIGHTER => Kind::Fighter,
        CATEGORY_CHARGE => Kind::Charge,
        CATEGORY_IMPLANT => Kind::Implant,
        _ => Kind::Other,
    }
}

/// Works out whether a type runs when it is turned on (§6.4), for [`TypeRecord::active`](crate::TypeRecord::active).
///
/// `effects` gives, for each `effectID` in the type's `dogmaEffects` (in `typeDogma.jsonl`),
/// that effect's `effectCategoryID` (in `dogmaEffects.jsonl`). A `HashMap<u32, u32>` or a
/// list of pairs both work.
///
/// A type runs when one of its effects is in category 1 (active) or 2 (target).
/// The SDE puts the `online` effect in category 1 too, but going online is not
/// running, so that one does not count.
///
/// ```
/// // Damage Control II: only online (16, category 1) and a passive effect.
/// assert!(!esf::sde::active([(16, 1), (2302, 0)]));
/// // 200mm AutoCannon II: online and projectileFired (34, category 2).
/// assert!(esf::sde::active([(16, 1), (34, 2)]));
/// ```
pub fn active(effects: impl IntoIterator<Item = (u32, u32)>) -> bool {
    effects.into_iter().any(|(effect_id, category)| {
        effect_id != EFFECT_ONLINE
            && (category == EFFECT_CATEGORY_ACTIVE || category == EFFECT_CATEGORY_TARGET)
    })
}

/// Reads how many fighters make up a full squadron, for [`TypeRecord::squadron_size`](crate::TypeRecord::squadron_size).
///
/// `attributes` gives, for each `attributeID` in the type's `dogmaAttributes`
/// (in `typeDogma.jsonl`), its `value`. A `HashMap<u32, f64>` or a list of pairs both
/// work. Returns `None` when the type has no `fighterSquadronMaxSize` (2215), which only
/// fighters have.
///
/// ```
/// assert_eq!(esf::sde::squadron_size([(9, 100.0), (2215, 6.0)]), Some(6));
/// ```
pub fn squadron_size(attributes: impl IntoIterator<Item = (u32, f64)>) -> Option<u32> {
    attributes
        .into_iter()
        .find(|&(attribute_id, _)| attribute_id == ATTRIBUTE_SQUADRON_SIZE)
        .map(|(_, value)| value as u32)
}

/// Finds which hull a tactical mode belongs to, for [`Catalog::modes`](crate::Catalog::modes).
///
/// The SDE does not link a mode to its hull. Instead, a mode's name starts with
/// its hull's name: "Hecate Sharpshooter Mode" belongs to the Hecate. This tries
/// the longest start of `mode_name` first, a whole word at a time.
///
/// `ship_names` is a [`name_index`] of every type in category `Ship` (6).
/// Returns the hull's type ID, or `None` when no ship name matches.
///
/// ```
/// use std::collections::HashMap;
///
/// let ships = HashMap::from([("hecate".to_string(), 35683)]);
/// assert_eq!(esf::sde::mode_hull("Hecate Sharpshooter Mode", &ships), Some(35683));
/// ```
pub fn mode_hull(mode_name: &str, ship_names: &HashMap<String, u32>) -> Option<u32> {
    let words: Vec<&str> = mode_name.split(' ').collect();
    (1..words.len())
        .rev()
        .find_map(|n| ship_names.get(&fold(&words[..n].join(" "))).copied())
}

/// Builds the name store that [`Catalog::type_by_name`](crate::Catalog::type_by_name) and
/// [`Catalog::attribute_by_name`](crate::Catalog::attribute_by_name) look names up in.
///
/// Give it every type, or every dogma attribute, including unpublished ones, and say how to
/// read the ID, the name and whether it is published from each. Each name is
/// case-folded with [`fold`], the same way the library does with the names in a fit.
/// Where names are shared, the published one wins, then the lowest ID (§4.2).
/// In the SDE, a type or attribute without `published` counts as not published.
///
/// Returns a map from case-folded name to ID.
///
/// ```
/// struct Named { id: u32, name: String, published: bool }
///
/// let types = [
///     Named { id: 50097, name: "Avatar".to_string(), published: false },
///     Named { id: 11567, name: "Avatar".to_string(), published: true },
/// ];
/// let index = esf::sde::name_index(&types, |t| t.id, |t| &t.name, |t| t.published);
/// assert_eq!(index["avatar"], 11567);
/// ```
pub fn name_index<T, S: AsRef<str>>(
    entries: impl IntoIterator<Item = T>,
    id: impl Fn(&T) -> u32,
    name: impl Fn(&T) -> S,
    published: impl Fn(&T) -> bool,
) -> HashMap<String, u32> {
    let mut sorted: Vec<(bool, u32, String)> = entries
        .into_iter()
        .map(|e| (!published(&e), id(&e), fold(name(&e).as_ref())))
        .collect();
    sorted.sort_unstable();
    let mut index = HashMap::with_capacity(sorted.len());
    for (_, id, name) in sorted {
        index.entry(name).or_insert(id);
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Named {
        id: u32,
        name: &'static str,
        published: bool,
    }

    #[test]
    fn kinds() {
        let cases: [(&str, u32, u32, &[u32], Kind); 18] = [
            ("Rifter", 6, 25, &[5779], Kind::Hull),
            ("Astrahus", 65, 1657, &[6565], Kind::Hull),
            ("Small Secure Container", 2, 340, &[649], Kind::Container),
            ("Hecate Sharpshooter Mode", 7, 1306, &[6010], Kind::Mode),
            (
                "Tengu Defensive - Covert Reconfiguration",
                32,
                954,
                &[896],
                Kind::Sub,
            ),
            ("200mm AutoCannon II", 7, 55, &[16, 12, 34], Kind::High),
            ("Medium Shield Extender II", 7, 38, &[13, 16], Kind::Mid),
            ("Damage Control II", 7, 60, &[11, 16], Kind::Low),
            (
                "Standup Ballistic Control System I",
                66,
                1429,
                &[11, 16],
                Kind::Low,
            ),
            (
                "Small Core Defense Field Extender I",
                7,
                774,
                &[446, 2663],
                Kind::Rig,
            ),
            ("Standup Cloning Center I", 66, 1321, &[16, 6306], Kind::Svc),
            ("module without a slot", 7, 1, &[16], Kind::Other),
            ("Hobgoblin II", 18, 100, &[10], Kind::Drone),
            ("Templar II", 87, 1652, &[6431], Kind::Fighter),
            ("Republic Fleet EMP S", 8, 83, &[596], Kind::Charge),
            (
                "Genolution Core Augmentation CA-1",
                20,
                1730,
                &[302],
                Kind::Implant,
            ),
            ("Strong Blue Pill Booster", 20, 303, &[2737], Kind::Booster),
            ("Veldspar", 25, 462, &[], Kind::Other),
        ];
        for (name, category_id, group_id, effect_ids, expected) in cases {
            let effect_ids = effect_ids.iter().copied();
            assert_eq!(kind(category_id, group_id, effect_ids), expected, "{name}");
        }
        assert_eq!(kind(7, 1, []), Kind::Other);
    }

    #[test]
    fn active_needs_an_active_or_target_effect() {
        assert!(!active([]));
        assert!(!active(HashMap::new()));
        assert!(!active([(16, 1), (2302, 0)]));
        assert!(active([(16, 1), (4, 1)]));
        assert!(active(HashMap::from([(16, 1), (3174, 2)])));
    }

    #[test]
    fn squadron_size_of_a_fighter() {
        assert_eq!(squadron_size([(9, 100.0), (2215, 6.0)]), Some(6));
        assert_eq!(squadron_size(HashMap::from([(9, 240.0)])), None);
        assert_eq!(squadron_size([]), None);
    }

    #[test]
    fn mode_hull_matches_the_longest_ship_name() {
        let ships = HashMap::from([
            ("hecate".to_string(), 35683),
            ("hecate navy".to_string(), 1),
        ]);
        assert_eq!(mode_hull("Hecate Sharpshooter Mode", &ships), Some(35683));
        assert_eq!(mode_hull("Hecate Navy Defense Mode", &ships), Some(1));
        assert_eq!(mode_hull("Hecate", &ships), None);
        assert_eq!(mode_hull("Rifter Defense Mode", &ships), None);
    }

    #[test]
    fn name_index_folds_names_and_picks_published_then_lowest_id() {
        let entries = [
            Named {
                id: 50097,
                name: "Avatar",
                published: false,
            },
            Named {
                id: 11567,
                name: "Avatar",
                published: true,
            },
            Named {
                id: 30,
                name: "Twin",
                published: false,
            },
            Named {
                id: 20,
                name: "Twin",
                published: false,
            },
            Named {
                id: 587,
                name: "Rifter",
                published: true,
            },
        ];
        let index = name_index(entries, |e| e.id, |e| e.name, |e| e.published);
        let expected = HashMap::from([
            ("avatar".to_string(), 11567),
            ("twin".to_string(), 20),
            ("rifter".to_string(), 587),
        ]);
        assert_eq!(index, expected);
    }

    #[test]
    fn name_index_takes_owned_names() {
        let index = name_index([(1, "Rifter")], |e| e.0, |e| e.1.to_string(), |_| true);
        assert_eq!(index["rifter"], 1);
    }
}
