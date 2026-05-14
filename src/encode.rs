//! ETF encoder

use thiserror::Error;

use crate::term::Term;

const NEW_FLOAT_EXT: u8 = 70;
const SMALL_INTEGER_EXT: u8 = 97;
const INTEGER_EXT: u8 = 98;
const SMALL_TUPLE_EXT: u8 = 104;
const LARGE_TUPLE_EXT: u8 = 105;
const NIL_EXT: u8 = 106;
const LIST_EXT: u8 = 108;
const BINARY_EXT: u8 = 109;
const SMALL_BIG_EXT: u8 = 110;
const SMALL_ATOM_EXT: u8 = 115;
const MAP_EXT: u8 = 116;
const VERSION_BYTE: u8 = 131;

#[derive(Debug, Error)]
pub enum EncodeError {
    #[error("atom byte length {byte_len} exceeds 254")]
    AtomTooLong { byte_len: usize },
    #[error("big-int digit count {digit_count} exceeds 8 (encoder cap)")]
    BigTooManyDigits { digit_count: usize },
    #[error("binary payload length {byte_len} exceeds u32::MAX")]
    BinaryTooLong { byte_len: usize },
    #[error("{kind} arity {arity} exceeds u32::MAX")]
    ContainerTooLong { kind: &'static str, arity: usize },
}

impl EncodeError {
    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::AtomTooLong { .. } => "atom_too_long",
            Self::BigTooManyDigits { .. } => "big_too_many_digits",
            Self::BinaryTooLong { .. } => "binary_too_long",
            Self::ContainerTooLong { .. } => "container_too_long",
        }
    }
}

pub fn encode(term: &Term) -> Result<Vec<u8>, EncodeError> {
    let mut output = Vec::new();
    output.push(VERSION_BYTE);
    encode_term(term, &mut output)?;
    Ok(output)
}

fn encode_term(term: &Term, output: &mut Vec<u8>) -> Result<(), EncodeError> {
    match term {
        Term::Atom(s) => encode_atom(s, output),
        Term::Integer(n) => {
            encode_integer(*n, output);
            Ok(())
        }
        Term::Float(f) => {
            encode_float(*f, output);
            Ok(())
        }
        Term::Big { sign, digits } => encode_big(*sign, digits, output),
        Term::Binary(bytes) => encode_binary(bytes, output),
        Term::Tuple(elems) => encode_tuple(elems, output),
        Term::List(elems) => encode_list(elems, output),
        Term::Map(entries) => encode_map(entries, output),
    }
}

fn encode_atom(s: &str, output: &mut Vec<u8>) -> Result<(), EncodeError> {
    let bytes = s.as_bytes();
    if bytes.len() > 254 {
        return Err(EncodeError::AtomTooLong {
            byte_len: bytes.len(),
        });
    }
    let len_u8 = u8::try_from(bytes.len()).expect("bytes.len() <= 254 fits in u8");
    output.push(SMALL_ATOM_EXT);
    output.push(len_u8);
    output.extend_from_slice(bytes);
    Ok(())
}

fn encode_integer(n: i32, output: &mut Vec<u8>) {
    if (0..=255).contains(&n) {
        let n_u8 = u8::try_from(n).expect("0..=255 fits in u8");
        output.push(SMALL_INTEGER_EXT);
        output.push(n_u8);
    } else {
        output.push(INTEGER_EXT);
        output.extend_from_slice(&n.to_be_bytes());
    }
}

fn encode_float(f: f64, output: &mut Vec<u8>) {
    output.push(NEW_FLOAT_EXT);
    output.extend_from_slice(&f.to_bits().to_be_bytes());
}

fn encode_big(sign: bool, digits: &[u8], output: &mut Vec<u8>) -> Result<(), EncodeError> {
    if digits.len() > 8 {
        return Err(EncodeError::BigTooManyDigits {
            digit_count: digits.len(),
        });
    }
    let n_u8 = u8::try_from(digits.len()).expect("digits.len() <= 8 fits in u8");
    output.push(SMALL_BIG_EXT);
    output.push(n_u8);
    output.push(u8::from(sign));
    output.extend_from_slice(digits);
    Ok(())
}

fn encode_binary(bytes: &[u8], output: &mut Vec<u8>) -> Result<(), EncodeError> {
    let Ok(len_u32) = u32::try_from(bytes.len()) else {
        return Err(EncodeError::BinaryTooLong {
            byte_len: bytes.len(),
        });
    };
    output.push(BINARY_EXT);
    output.extend_from_slice(&len_u32.to_be_bytes());
    output.extend_from_slice(bytes);
    Ok(())
}

fn encode_tuple(elems: &[Term], output: &mut Vec<u8>) -> Result<(), EncodeError> {
    let arity = elems.len();
    if let Ok(arity_u8) = u8::try_from(arity) {
        output.push(SMALL_TUPLE_EXT);
        output.push(arity_u8);
    } else if let Ok(arity_u32) = u32::try_from(arity) {
        output.push(LARGE_TUPLE_EXT);
        output.extend_from_slice(&arity_u32.to_be_bytes());
    } else {
        return Err(EncodeError::ContainerTooLong {
            kind: "tuple",
            arity,
        });
    }
    for elem in elems {
        encode_term(elem, output)?;
    }
    Ok(())
}

fn encode_list(elems: &[Term], output: &mut Vec<u8>) -> Result<(), EncodeError> {
    if elems.is_empty() {
        output.push(NIL_EXT);
        return Ok(());
    }
    let Ok(arity_u32) = u32::try_from(elems.len()) else {
        return Err(EncodeError::ContainerTooLong {
            kind: "list",
            arity: elems.len(),
        });
    };
    output.push(LIST_EXT);
    output.extend_from_slice(&arity_u32.to_be_bytes());
    for elem in elems {
        encode_term(elem, output)?;
    }
    output.push(NIL_EXT);
    Ok(())
}

fn encode_map(entries: &[(Term, Term)], output: &mut Vec<u8>) -> Result<(), EncodeError> {
    let Ok(arity_u32) = u32::try_from(entries.len()) else {
        return Err(EncodeError::ContainerTooLong {
            kind: "map",
            arity: entries.len(),
        });
    };
    output.push(MAP_EXT);
    output.extend_from_slice(&arity_u32.to_be_bytes());
    for (k, v) in entries {
        encode_term(k, output)?;
        encode_term(v, output)?;
    }
    Ok(())
}
