use crate::encode::{EncodeError, encode_term};
use crate::term::Term;

const VERSION_BYTE: u8 = 131;
const MAP_EXT: u8 = 116;

pub enum SplicedValue<'a> {
    Owned(Term),
    PreEncoded(&'a [u8]),
}

pub fn splice_map(entries: &[(Term, SplicedValue<'_>)]) -> Result<Vec<u8>, EncodeError> {
    let Ok(arity_u32) = u32::try_from(entries.len()) else {
        return Err(EncodeError::ContainerTooLong {
            kind: "map",
            arity: entries.len(),
        });
    };
    let mut out = Vec::new();
    out.push(VERSION_BYTE);
    out.push(MAP_EXT);
    out.extend_from_slice(&arity_u32.to_be_bytes());
    for (key, value) in entries {
        encode_term(key, &mut out)?;
        match value {
            SplicedValue::Owned(t) => encode_term(t, &mut out)?,
            SplicedValue::PreEncoded(bytes) => out.extend_from_slice(bytes),
        }
    }
    Ok(out)
}
