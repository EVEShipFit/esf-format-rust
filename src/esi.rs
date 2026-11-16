//! Helpers to turn an ESI `location_flag` into an [`Entry::location`](crate::Entry::location)
//! and [`Entry::slot`](crate::Entry::slot).
//!
//! ESI gives a `location_flag` for each item in a ship, in the assets and fittings endpoints.
//! Use both helpers on the same flag:
//!
//! ```
//! # let (type_id, flag) = (2889, "HiSlot2");
//! let entry = esf::Entry {
//!     type_id: Some(type_id),
//!     location: Some(esf::esi::location(flag).unwrap_or(esf::Location::Cargo)),
//!     slot: esf::esi::slot(flag),
//!     ..esf::Entry::default()
//! };
//! ```
//!
//! Only the flags of things inside a ship are known. For any other flag, like `Hangar`, both
//! return `None`. The example above puts those items in the cargo hold.

use crate::Location;

const RACKS: [(&str, Location, u32); 7] = [
    ("HiSlot", Location::High, 8),
    ("MedSlot", Location::Mid, 8),
    ("LoSlot", Location::Low, 8),
    ("RigSlot", Location::Rig, 8),
    ("SubSystemSlot", Location::Sub, 8),
    ("ServiceSlot", Location::Svc, 8),
    ("FighterTube", Location::Tube, 5),
];

const HOLDS: [(&str, Location); 22] = [
    ("Cargo", Location::Cargo),
    ("DroneBay", Location::Bay),
    ("FighterBay", Location::Bay),
    ("SpecializedAmmoHold", Location::Ammo),
    ("BoosterBay", Location::Booster),
    ("SpecializedCommandCenterHold", Location::Command),
    ("CorpseBay", Location::Corpse),
    ("MobileDepotHold", Location::Depot),
    ("ExpeditionHold", Location::Expedition),
    ("FleetHangar", Location::Fleet),
    ("FrigateEscapeBay", Location::Frigate),
    ("SpecializedFuelBay", Location::Fuel),
    ("SpecializedGasHold", Location::Gas),
    ("SpecializedIceHold", Location::Ice),
    ("InfrastructureHangar", Location::Infrastructure),
    ("ShipHangar", Location::Maintenance),
    ("SpecializedMineralHold", Location::Mineral),
    ("SpecializedOreHold", Location::Mining),
    ("MoonMaterialBay", Location::Moon),
    ("SpecializedPlanetaryCommoditiesHold", Location::Planetary),
    ("QuafeBay", Location::Quafe),
    ("SubSystemBay", Location::Subsystem),
];

/// Gives the [`Location`] for an ESI `location_flag`.
///
/// A slot flag gives its rack: `HiSlot2` gives [`Location::High`], and `FighterTube0` gives
/// [`Location::Tube`]. `DroneBay` and `FighterBay` give [`Location::Bay`]. In a fitting from
/// ESI, `DroneBay` also holds the drones that are out in space. Leave
/// [`Entry::location`](crate::Entry::location) `None` for those.
///
/// Returns `None` for a flag that is not inside a ship, like `Hangar`.
pub fn location(flag: &str) -> Option<Location> {
    rack_slot(flag).map(|(location, _)| location).or_else(|| {
        HOLDS
            .iter()
            .find(|&&(name, _)| name == flag)
            .map(|&(_, location)| location)
    })
}

/// Gives the [`Entry::slot`](crate::Entry::slot) for an ESI `location_flag`, counting from 1.
///
/// ESI counts slots from 0, so `HiSlot2` is slot 3, and `FighterTube0` is tube 1.
/// Returns `None` for a flag that is not a slot, like `Cargo` or `Hangar`.
pub fn slot(flag: &str) -> Option<u32> {
    rack_slot(flag).map(|(_, slot)| slot)
}

fn rack_slot(flag: &str) -> Option<(Location, u32)> {
    RACKS.into_iter().find_map(|(prefix, location, size)| {
        if let Some(index) = flag.strip_prefix(prefix)
            && let [digit @ b'0'..=b'9'] = index.as_bytes()
            && let index = u32::from(digit - b'0')
            && index < size
        {
            Some((location, index + 1))
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots() {
        let cases = [
            ("HiSlot0", Location::High, 1),
            ("MedSlot7", Location::Mid, 8),
            ("LoSlot2", Location::Low, 3),
            ("RigSlot1", Location::Rig, 2),
            ("SubSystemSlot3", Location::Sub, 4),
            ("ServiceSlot5", Location::Svc, 6),
            ("FighterTube4", Location::Tube, 5),
        ];
        for (flag, want, n) in cases {
            assert_eq!(location(flag), Some(want), "{flag}");
            assert_eq!(slot(flag), Some(n), "{flag}");
        }
    }

    #[test]
    fn every_hold() {
        let flags = [
            "Cargo",
            "DroneBay",
            "SpecializedAmmoHold",
            "BoosterBay",
            "SpecializedCommandCenterHold",
            "CorpseBay",
            "MobileDepotHold",
            "ExpeditionHold",
            "FleetHangar",
            "FrigateEscapeBay",
            "SpecializedFuelBay",
            "SpecializedGasHold",
            "SpecializedIceHold",
            "InfrastructureHangar",
            "ShipHangar",
            "SpecializedMineralHold",
            "SpecializedOreHold",
            "MoonMaterialBay",
            "SpecializedPlanetaryCommoditiesHold",
            "QuafeBay",
            "SubSystemBay",
        ];
        let holds: Vec<Location> = Location::ALL
            .into_iter()
            .filter(|l| !Location::RACKS.contains(l))
            .collect();
        let got: Vec<Location> = flags.iter().map(|f| location(f).unwrap()).collect();
        assert_eq!(got, holds);
        assert!(flags.iter().all(|f| slot(f).is_none()));
        assert_eq!(location("FighterBay"), Some(Location::Bay));
    }

    #[test]
    fn unknown() {
        for flag in [
            "Hangar",
            "AutoFit",
            "HiSlot8",
            "FighterTube5",
            "hislot0",
            "HiSlot01",
            "HiSlot0\n",
            "toString",
            "",
        ] {
            assert_eq!(location(flag), None, "{flag}");
            assert_eq!(slot(flag), None, "{flag}");
        }
    }
}
