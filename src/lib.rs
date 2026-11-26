#![doc = include_str!("../README.md")]

mod base64;
mod binary;
mod canonical;
mod catalog;
mod error;
pub mod esi;
mod fold_table;
mod model;
mod names;
mod number;
mod resolve;
pub mod sde;
mod source;
mod text;
mod wire;

pub use base64::{from_base64url, to_base64url};
pub use catalog::{AttributeRecord, Catalog, Kind, MutaplasmidRecord, TypeRecord};
pub use error::{Error, ErrorKind};
pub use model::{Document, Entry, EsfFit, Extra, Location, State};
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
    let blocks = text::parse(text.as_bytes()).map(|(blocks, _)| blocks);
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
    read(data.as_ref(), catalog, false).map(|document| document.fits)
}

/// Read a text or binary document, in canonical form, and keep the data that is not a fit.
///
/// The fits are the same as [`load`] returns. [`Document::extra`] holds what `load` skips:
/// every text block whose name is not `esf`, like `%fleet/1`, and every
/// length-delimited field of a binary document other than its fits. Other binary fields are
/// still skipped. A `%esf` header with a version other than 1 is an error, as with `load`.
/// So is a block whose version is more than 4,294,967,295, which `load` skips.
pub fn load_document(data: impl AsRef<[u8]>, catalog: &dyn Catalog) -> Result<Document, Error> {
    read(data.as_ref(), catalog, true)
}

fn read(data: &[u8], catalog: &dyn Catalog, keep: bool) -> Result<Document, Error> {
    if data.is_empty() {
        return Err(Error::format("the document is empty"));
    }
    if is_text(data) {
        let (blocks, others) = text::parse(data)?;
        let fits = canonical(&blocks, catalog, None).map(|fits| to_model(&fits))?;
        let extra = match keep {
            true => others.iter().map(text_extra).collect::<Result<_, _>>()?,
            false => Vec::new(),
        };
        return Ok(Document { fits, extra });
    }
    let (fits, extra) = binary::decode(data)?;
    let fits = checked(&fits, catalog, to_model)?;
    Ok(Document { fits, extra })
}

fn text_extra(block: &text::OtherBlock<'_>) -> Result<Extra, Error> {
    let version = block.version.parse().map_err(|_| {
        Error::format(format!("the version of %{} is too large", block.name)).at(block.line)
    })?;
    Ok(Extra::Text {
        name: block.name.to_string(),
        version,
        text: block.text.to_string(),
    })
}

/// The bytes of one field of a binary document, to append to what [`save_binary`] returns.
///
/// [`load_document`] gives it back as an [`Extra::Binary`] with this `field` and `bytes`.
/// Use a field number that is not in the esf/1 schema, so not 1.
///
/// # Panics
///
/// If `field` is 0, 1 (the fits), or more than 536,870,911, the largest field number
/// Protocol Buffers allows.
pub fn binary_field(field: u32, bytes: &[u8]) -> Vec<u8> {
    assert!(
        (2..=wire::MAX_FIELD).contains(&field),
        "binary field {field} is not a free field number"
    );
    let mut out = Vec::new();
    wire::record(&mut out, field, bytes);
    out
}

/// Write fits as canonical text (§8).
pub fn save(fits: &[EsfFit], catalog: &dyn Catalog) -> Result<String, Error> {
    checked(fits, catalog, canonical::render)
}

/// Write fits in the binary form (§10).
pub fn save_binary(fits: &[EsfFit], catalog: &dyn Catalog) -> Result<Vec<u8>, Error> {
    checked(fits, catalog, |fits| binary::encode(&to_model(fits)))
}
