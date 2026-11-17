//! What esf/1 needs to know about types and attributes (SPEC §5, §6).

use crate::{Error, number};

/// What a type is, which decides where it goes in a fit (esf/1 spec §5).
///
/// Check the group rules first ([`Mode`](Kind::Mode), [`Container`](Kind::Container),
/// [`Booster`](Kind::Booster)), then the category rules.
/// [`sde::kind`](crate::sde::kind) works this out from the SDE for you.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A ship or a structure: SDE category `Ship` (6) or `Structure` (65).
    Hull,
    /// A container that can carry another fit, like a Small Secure Container.
    ///
    /// SDE group `Cargo Container` (12), `Secure Cargo Container` (340),
    /// `Audit Log Secure Container` (448) or `Freight Container` (649).
    Container,
    /// A tactical mode, like the Hecate's Sharpshooter Mode: SDE group `Ship Modifiers` (1306).
    Mode,
    /// A subsystem, like on a Tengu: SDE category `Subsystem` (32).
    Sub,
    /// A high slot module, like a gun: SDE category `Module` (7) or `Structure Module` (66),
    /// with effect `hiPower` (12).
    High,
    /// A mid slot module, like a shield extender: category `Module` or `Structure Module`,
    /// with effect `medPower` (13).
    Mid,
    /// A low slot module, like a damage control: category `Module` or `Structure Module`,
    /// with effect `loPower` (11).
    Low,
    /// A rig: category `Module` or `Structure Module`, with effect `rigSlot` (2663).
    Rig,
    /// A structure service module: category `Module` or `Structure Module`, with effect
    /// `serviceSlot` (6306).
    Svc,
    /// A drone: SDE category `Drone` (18).
    Drone,
    /// A fighter: SDE category `Fighter` (87).
    Fighter,
    /// Ammunition, a script or anything else loaded into a module: SDE category `Charge` (8).
    Charge,
    /// An implant: SDE category `Implant` (20), except the boosters in it.
    Implant,
    /// A booster, also called a drug: SDE group `Booster` (303), inside category `Implant`.
    Booster,
    /// Anything else, like ore or a module without a slot. It goes in the cargo.
    Other,
}

impl Kind {
    /// Whether a type of this kind can be the [`hull`](crate::EsfFit::hull) of a fit: true
    /// for [`Hull`](Kind::Hull) and [`Container`](Kind::Container) only.
    pub fn can_be_hull(self) -> bool {
        matches!(self, Kind::Hull | Kind::Container)
    }
}

/// One type, like "Rifter" or "200mm AutoCannon II", as the library needs it.
///
/// With the SDE, most fields come from `types.jsonl`. [`kind`](TypeRecord::kind),
/// [`active`](TypeRecord::active) and [`squadron_size`](TypeRecord::squadron_size) also
/// need `groups.jsonl`, `typeDogma.jsonl` and `dogmaEffects.jsonl`, through the helpers in
/// [`sde`](crate::sde). Include every type in `types.jsonl`. A type with no row in
/// `typeDogma.jsonl` has no effects and no attributes: give the helpers an empty list or map.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeRecord {
    /// The type ID, like 587 for the Rifter. In the SDE this is `_key`.
    pub id: u32,
    /// The English name, like "Rifter". In the SDE this is `name.en`.
    pub name: String,
    /// What it is, which decides where it goes in a fit. Use [`sde::kind`](crate::sde::kind)
    /// to work this out.
    pub kind: Kind,
    /// Whether the type runs when it is turned on, like a gun or a shield booster.
    ///
    /// A plate or a damage control only goes online, so it is false.
    /// Use [`sde::active`](crate::sde::active) to work this out from the type's effects.
    pub active: bool,
    /// How much the type holds, in m³; for a module, how much ammunition fits in it.
    ///
    /// In the SDE this is `capacity`. Use 0 when the type has none.
    pub capacity: f64,
    /// How much room one of it takes, in m³. In the SDE this is `volume`. Use 0 when the
    /// type has none.
    pub volume: f64,
    /// For a fighter, how many make up a full squadron. `None` for anything else.
    ///
    /// In the SDE this is the attribute `fighterSquadronMaxSize`;
    /// [`sde::squadron_size`](crate::sde::squadron_size) reads it for you.
    pub squadron_size: Option<u32>,
}

/// One dogma attribute, like "speedFactor", as the library needs it.
///
/// With the SDE, both fields come from `dogmaAttributes.jsonl`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttributeRecord {
    /// The attribute ID, like 20 for speedFactor. In the SDE this is `_key`.
    pub id: u32,
    /// The name, like "speedFactor". In the SDE this is `name`; it is not translated.
    pub name: String,
}

/// One mutaplasmid that can be applied to a base type, like "Unstable 5MN Microwarpdrive
/// Mutaplasmid".
///
/// With the SDE, these come from `dynamicItemAttributes.jsonl`. Each entry there is one
/// mutaplasmid. Its `inputOutputMapping` is a list; the base types it applies to are all the
/// `applicableTypes` across that list.
#[derive(Clone, Debug, PartialEq)]
pub struct MutaplasmidRecord {
    /// The mutaplasmid's own type: the type whose ID is the entry's `_key`.
    pub mutaplasmid: TypeRecord,
    /// The attributes the mutaplasmid changes, each with the base type's value before the
    /// change, as (attribute ID, value).
    ///
    /// The attribute IDs are the `_key` of each item in the entry's `attributeIDs`; its `min`
    /// and `max` are not needed. Each value is the base type's value of that attribute in
    /// `typeDogma.jsonl`. When the base type does not have it, use the attribute's
    /// `defaultValue` in `dogmaAttributes.jsonl`, or 0 when that is missing too.
    pub rolls: Vec<(u32, f64)>,
}

/// Where the library looks up game data. You write this, on top of your copy of the SDE.
///
/// The library holds no game data itself. Whenever it meets a name or an ID in a fit,
/// it asks the catalog what that is. [`sde`](crate::sde) has helpers for the parts that
/// take more than a lookup.
///
/// The `§` numbers point to sections of the
/// [esf/1 spec](https://github.com/EVEShipFit/esf-format/blob/main/SPEC.md).
pub trait Catalog {
    /// Finds a type by its name.
    ///
    /// The name comes from a fit, so a user may have typed it in any case.
    /// The library already case-folded it with [`fold`](crate::fold), so "Rifter", "RIFTER"
    /// and "rifter" all reach you as "rifter". Case folding is like lowercase, but
    /// gives the same result in every language, so use [`fold`](crate::fold), not
    /// `to_lowercase()`. Store your type names the same way, by calling
    /// [`fold`](crate::fold) on each one, and look this name up in that store.
    ///
    /// Include unpublished types too. Some names belong to more than one type.
    /// Then return the published one; if that is still more than one, the lowest ID.
    /// [`sde::name_index`](crate::sde::name_index) builds this store for you.
    ///
    /// Return `None` if no type has this name.
    fn type_by_name(&self, folded_name: &str) -> Option<&TypeRecord>;

    /// Finds a type by its ID. Return `None` if there is no type with this ID.
    fn type_by_id(&self, type_id: u32) -> Option<&TypeRecord>;

    /// Finds an attribute by its name, like "speedfactor".
    ///
    /// Used to read attribute overrides, like the rolls on an abyssal module.
    ///
    /// This works the same way as [`type_by_name`](Catalog::type_by_name): the name is
    /// already case-folded with [`fold`](crate::fold), unpublished attributes count too, and
    /// where names are shared, the published one wins, then the lowest ID.
    /// [`sde::name_index`](crate::sde::name_index) builds this store for you.
    ///
    /// Return `None` if no attribute has this name.
    fn attribute_by_name(&self, folded_name: &str) -> Option<&AttributeRecord>;

    /// Finds an attribute by its ID.
    ///
    /// Used to write attribute overrides, like the rolls on an abyssal module.
    ///
    /// Return `None` if there is no attribute with this ID.
    fn attribute_by_id(&self, attribute_id: u32) -> Option<&AttributeRecord>;

    /// The tactical modes a hull can be in, like the Hecate's Defense, Propulsion and
    /// Sharpshooter Mode.
    ///
    /// Return an empty slice for a hull without tactical modes. In the SDE, modes are the
    /// types of kind [`Kind::Mode`]; [`sde::mode_hull`](crate::sde::mode_hull) tells you which
    /// hull each one belongs to.
    ///
    /// Leave this out and no hull has tactical modes.
    fn modes(&self, _hull_id: u32) -> &[TypeRecord] {
        &[]
    }

    /// The mutaplasmids that can be applied to a type. See [`MutaplasmidRecord`] for where
    /// they come from.
    ///
    /// Return an empty slice for a type no mutaplasmid applies to.
    ///
    /// Leave this out and no type has mutaplasmids.
    fn mutaplasmids(&self, _type_id: u32) -> &[MutaplasmidRecord] {
        &[]
    }
}

pub(crate) fn attribute_of(
    catalog: &dyn Catalog,
    attribute_id: u32,
) -> Result<&AttributeRecord, Error> {
    catalog
        .attribute_by_id(attribute_id)
        .ok_or_else(|| Error::new(format!("the catalog has no attribute {attribute_id}")))
}

/// How many charges fit in an item, by the shortest decimals of both volumes (§6.5).
pub(crate) fn full_load(item: &TypeRecord, charge: &TypeRecord) -> Option<u64> {
    let room = |v: f64| v.is_finite() && v > 0.0;
    if !room(item.capacity) || !room(charge.volume) {
        return None;
    }
    let (capacity, capacity_exponent) = decimal(item.capacity);
    let (volume, volume_exponent) = decimal(charge.volume);
    let shift = capacity_exponent - volume_exponent;
    let scale = 10u128.checked_pow(shift.unsigned_abs());
    let quotient = if shift >= 0 {
        scale
            .and_then(|s| capacity.checked_mul(s))
            .map(|c| c / volume)
    } else {
        Some(
            scale
                .and_then(|s| volume.checked_mul(s))
                .map_or(0, |v| capacity / v),
        )
    };
    Some(quotient.map_or(u64::MAX, |q| q.try_into().unwrap_or(u64::MAX)))
}

/// A positive float as its shortest decimal: `digits × 10^exponent`.
fn decimal(value: f64) -> (u128, i32) {
    let (digits, exponent) = number::shortest(value);
    (digits.parse().expect("at most 17 digits"), exponent)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(capacity: f64, volume: f64) -> TypeRecord {
        TypeRecord {
            id: 1,
            name: String::new(),
            kind: Kind::Other,
            active: false,
            capacity,
            volume,
            squadron_size: None,
        }
    }

    fn quotient(capacity: f64, volume: f64) -> Option<u64> {
        full_load(&record(capacity, 0.0), &record(0.0, volume))
    }

    #[test]
    fn full_load_is_exact() {
        assert_eq!(quotient(0.3, 0.1), Some(3));
        assert_eq!(quotient(10.0, 0.1), Some(100));
        assert_eq!(quotient(400.0, 0.0125), Some(32000));
        assert_eq!(quotient(2.3, 0.05), Some(46));
        assert_eq!(quotient(1.0, 3.0), Some(0));
        assert_eq!(quotient(1.0, 2f64.powi(-24)), Some(16777215));
        assert_eq!(quotient(1000.0, 1e5), Some(0));
        assert_eq!(quotient(1e300, 1e-300), Some(u64::MAX));
        assert_eq!(quotient(1e30, 1.0), Some(u64::MAX));
        assert_eq!(quotient(1e-300, 1e300), Some(0));
        assert_eq!(quotient(1.0, 1e-30), Some(u64::MAX));
    }

    #[test]
    fn full_load_needs_room() {
        assert_eq!(quotient(0.0, 1.0), None);
        assert_eq!(quotient(1.0, 0.0), None);
        assert_eq!(quotient(f64::NAN, 1.0), None);
        assert_eq!(quotient(1.0, f64::INFINITY), None);
    }
}
