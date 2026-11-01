//! What esf/1 needs to know about types and attributes (SPEC §5, §6).

use crate::{Error, number};

/// What a type is (§5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Hull,
    Container,
    Mode,
    Sub,
    High,
    Mid,
    Low,
    Rig,
    Svc,
    Drone,
    Fighter,
    Charge,
    Implant,
    Booster,
    Other,
}

/// A type, as the Catalog knows it.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeRecord {
    pub id: u32,
    pub name: String,
    pub kind: Kind,
    pub active: bool,
    pub capacity: f64,
    pub volume: f64,
    pub squadron_size: Option<u32>,
}

/// An attribute, as the Catalog knows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttributeRecord {
    pub id: u32,
    pub name: String,
}

/// A mutaplasmid for one base type, with that type's value of each attribute it rolls.
#[derive(Clone, Debug, PartialEq)]
pub struct MutaplasmidRecord {
    pub mutaplasmid: TypeRecord,
    pub rolls: Vec<(u32, f64)>,
}

/// Types and attributes, by folded name (§6.10) or by ID.
pub trait Catalog {
    fn type_by_name(&self, folded: &str) -> Option<&TypeRecord>;

    fn type_by_id(&self, type_id: u32) -> Option<&TypeRecord>;

    fn attribute_by_name(&self, folded: &str) -> Option<&AttributeRecord>;

    fn attribute_by_id(&self, attribute_id: u32) -> Option<&AttributeRecord>;

    /// The tactical modes of a hull.
    fn modes(&self, _hull_id: u32) -> &[TypeRecord] {
        &[]
    }

    /// The mutaplasmids that apply to a type.
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
