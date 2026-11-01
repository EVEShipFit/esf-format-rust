#![doc = include_str!("../README.md")]

mod base64;
mod binary;
mod canonical;
mod catalog;
mod error;
mod fold_table;
mod model;
mod names;
mod number;
mod resolve;
mod source;
mod text;
mod wire;

pub use base64::{from_base64url, to_base64url};
pub use catalog::{AttributeRecord, Catalog, Kind, MutaplasmidRecord, TypeRecord};
pub use error::Error;
pub use model::{Entry, EsfFit, Location, State};
pub use names::fold;

use canonical::CanonFit;
use text::FitBlock;

/// Whether a document is text, by its first character (§10).
pub fn is_text(data: impl AsRef<[u8]>) -> bool {
    matches!(data.as_ref().first(), Some(b'%' | b' '))
}

fn canonical<'a>(
    blocks: &'a [FitBlock<'a>],
    catalog: &'a dyn Catalog,
    given: Option<&[EsfFit]>,
) -> Result<Vec<CanonFit<'a>>, Error> {
    let fits = resolve::resolve(blocks, catalog)?;
    if let Some(given) = given {
        source::verify(&fits, given)?;
    }
    fits.iter()
        .map(|fit| canonical::canonical_fit(&fits, fit, catalog))
        .collect()
}

/// Check fits by the text they stand for, then hand their canonical form to `finish`.
fn checked<T>(
    fits: &[EsfFit],
    catalog: &dyn Catalog,
    finish: impl FnOnce(&[CanonFit<'_>]) -> T,
) -> Result<T, Error> {
    let (text, lines) = source::source(fits, catalog)?;
    let blocks = text::parse(text.as_bytes());
    let result =
        blocks.and_then(|blocks| canonical(&blocks, catalog, Some(fits)).map(|c| finish(&c)));
    result.map_err(|e| match e.line {
        Some(line) => {
            let (fit, entry) = lines[line - 1];
            e.in_fit(fit, entry)
        }
        None => e,
    })
}

fn to_model(fits: &[CanonFit<'_>]) -> Vec<EsfFit> {
    fits.iter().map(CanonFit::to_model).collect()
}

/// Read a text or binary document, in canonical form.
pub fn load(data: impl AsRef<[u8]>, catalog: &dyn Catalog) -> Result<Vec<EsfFit>, Error> {
    let data = data.as_ref();
    if data.is_empty() {
        return Err(Error::new("the document is empty"));
    }
    if is_text(data) {
        let blocks = text::parse(data)?;
        return canonical(&blocks, catalog, None).map(|fits| to_model(&fits));
    }
    checked(&binary::decode(data)?, catalog, to_model)
}

/// Write fits as canonical text (§8).
pub fn save(fits: &[EsfFit], catalog: &dyn Catalog) -> Result<String, Error> {
    checked(fits, catalog, canonical::render)
}

/// Write fits in the binary form (§10).
pub fn save_binary(fits: &[EsfFit], catalog: &dyn Catalog) -> Result<Vec<u8>, Error> {
    checked(fits, catalog, |fits| binary::encode(&to_model(fits)))
}
