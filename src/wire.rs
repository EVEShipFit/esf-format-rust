//! The Protocol Buffers wire format.

use crate::Error;

const VARINT: u8 = 0;
const I64: u8 = 1;
const LEN: u8 = 2;
const SGROUP: u8 = 3;
const EGROUP: u8 = 4;
const I32: u8 = 5;

fn invalid(why: &str) -> Error {
    Error::new(format!("binary: not valid Protocol Buffers ({why})"))
}

pub(crate) fn varint(out: &mut Vec<u8>, mut value: u64) {
    while value > 0x7F {
        out.push(value as u8 | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

pub(crate) fn uint(out: &mut Vec<u8>, field: u32, value: Option<u32>) {
    if let Some(value) = value {
        varint(out, u64::from(field << 3 | u32::from(VARINT)));
        varint(out, u64::from(value));
    }
}

pub(crate) fn record(out: &mut Vec<u8>, field: u32, payload: &[u8]) {
    varint(out, u64::from(field << 3 | u32::from(LEN)));
    varint(out, payload.len() as u64);
    out.extend_from_slice(payload);
}

/// One value, by its wire type.
pub(crate) enum Value<'a> {
    Varint(u64),
    I64([u8; 8]),
    Len(&'a [u8]),
    I32,
    Group,
}

pub(crate) struct Reader<'a> {
    data: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data }
    }

    pub fn done(&self) -> bool {
        self.data.is_empty()
    }

    fn take(&mut self, n: u64) -> Result<&'a [u8], Error> {
        let n = usize::try_from(n).ok().filter(|&n| n <= self.data.len());
        let (taken, rest) = self.data.split_at(n.ok_or_else(|| invalid("cut short"))?);
        self.data = rest;
        Ok(taken)
    }

    pub fn varint(&mut self) -> Result<u64, Error> {
        let mut value = 0;
        for shift in (0..70).step_by(7) {
            let byte = self.take(1)?[0];
            value |= u64::from(byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(invalid("varint is too long"))
    }

    fn key(&mut self) -> Result<(u32, u8), Error> {
        let key = self.varint()?;
        let (field, wire) = (key >> 3, (key & 7) as u8);
        if field == 0 || key > u64::from(u32::MAX) || wire > I32 {
            return Err(invalid(&format!("bad key {key}")));
        }
        Ok((field as u32, wire))
    }

    fn value(&mut self, field: u32, wire: u8) -> Result<Value<'a>, Error> {
        Ok(match wire {
            VARINT => Value::Varint(self.varint()?),
            I64 => Value::I64(self.take(8)?.try_into().expect("took 8 bytes")),
            I32 => {
                self.take(4)?;
                Value::I32
            }
            LEN => {
                let len = self.varint()?;
                Value::Len(self.take(len)?)
            }
            SGROUP => {
                self.skip_group(field)?;
                Value::Group
            }
            _ => return Err(invalid("unmatched end group")),
        })
    }

    fn skip_group(&mut self, field: u32) -> Result<(), Error> {
        let mut open = vec![field];
        while let Some(&innermost) = open.last() {
            match self.key()? {
                (inner, SGROUP) => open.push(inner),
                (inner, EGROUP) if inner == innermost => _ = open.pop(),
                (_, EGROUP) => return Err(invalid("unmatched end group")),
                (inner, wire) => _ = self.value(inner, wire)?,
            }
        }
        Ok(())
    }

    /// The next field of the message, and its value.
    pub fn field(&mut self) -> Result<(u32, Value<'a>), Error> {
        let (field, wire) = self.key()?;
        Ok((field, self.value(field, wire)?))
    }
}

pub(crate) fn unpack_varints(payload: &[u8]) -> Result<Vec<u64>, Error> {
    let mut reader = Reader::new(payload);
    let mut out = Vec::new();
    while !reader.done() {
        out.push(reader.varint()?);
    }
    Ok(out)
}

pub(crate) fn unpack_doubles(payload: &[u8]) -> Result<Vec<f64>, Error> {
    if !payload.len().is_multiple_of(8) {
        return Err(invalid("packed doubles cut short"));
    }
    Ok(payload
        .as_chunks()
        .0
        .iter()
        .map(|&b| f64::from_le_bytes(b))
        .collect())
}
