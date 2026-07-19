use std::string::FromUtf8Error;

use serde_json::{Number, Value};
use thiserror::Error;

use crate::encode::{
    BINARY_EXT, EncodeError, INTEGER_EXT, LIST_EXT, MAP_EXT, NEW_FLOAT_EXT, NIL_EXT,
    SMALL_ATOM_EXT, SMALL_BIG_EXT, SMALL_INTEGER_EXT, VERSION_BYTE,
};
use crate::term::Term;

#[derive(Debug, Error)]
pub enum ConvertError {
    #[error("Term::Binary payload was not valid UTF-8: {source}")]
    BinaryInvalidUtf8 {
        #[source]
        source: FromUtf8Error,
    },
    #[error("Term::Map key was not a string-shaped term (kind: {key_kind})")]
    NonStringMapKey { key_kind: &'static str },
    #[error("Term::Float was non-finite and cannot be expressed as JSON Number")]
    NonFiniteFloat,
}

impl ConvertError {
    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::BinaryInvalidUtf8 { .. } => "binary_invalid_utf8",
            Self::NonStringMapKey { .. } => "non_string_map_key",
            Self::NonFiniteFloat => "non_finite_float",
        }
    }
}

pub fn to_value(term: &Term) -> Result<Value, ConvertError> {
    match term {
        Term::Atom(s) => Ok(atom_to_value(s)),
        Term::Integer(n) => Ok(Value::Number(Number::from(*n))),
        Term::Float(f) => float_to_value(*f),
        Term::Big { sign, digits } => Ok(big_to_value(*sign, digits)),
        Term::Binary(bytes) => binary_to_value(bytes),
        Term::Tuple(elems) | Term::List(elems) => sequence_to_value(elems),
        Term::Map(entries) => map_to_value(entries),
    }
}

pub fn from_value(value: &Value) -> Term {
    match value {
        Value::Null => Term::Atom("nil".to_string()),
        Value::Bool(true) => Term::Atom("true".to_string()),
        Value::Bool(false) => Term::Atom("false".to_string()),
        Value::Number(n) => number_to_term(n),
        Value::String(s) => Term::Binary(s.as_bytes().to_vec()),
        Value::Array(arr) => Term::List(arr.iter().map(from_value).collect()),
        Value::Object(obj) => Term::Map(
            obj.iter()
                .map(|(k, v)| (Term::Binary(k.as_bytes().to_vec()), from_value(v)))
                .collect(),
        ),
    }
}

fn atom_to_value(s: &str) -> Value {
    match s {
        "nil" | "null" => Value::Null,
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        other => Value::String(other.to_string()),
    }
}

fn float_to_value(f: f64) -> Result<Value, ConvertError> {
    Number::from_f64(f)
        .map(Value::Number)
        .ok_or(ConvertError::NonFiniteFloat)
}

fn binary_to_value(bytes: &[u8]) -> Result<Value, ConvertError> {
    String::from_utf8(bytes.to_vec())
        .map(Value::String)
        .map_err(|source| ConvertError::BinaryInvalidUtf8 { source })
}

fn sequence_to_value(elems: &[Term]) -> Result<Value, ConvertError> {
    let mut arr = Vec::with_capacity(elems.len());
    for e in elems {
        arr.push(to_value(e)?);
    }
    Ok(Value::Array(arr))
}

fn map_to_value(entries: &[(Term, Term)]) -> Result<Value, ConvertError> {
    let mut obj = serde_json::Map::new();
    for (k, v) in entries {
        let key_str = match k {
            Term::Binary(bytes) => String::from_utf8(bytes.clone())
                .map_err(|source| ConvertError::BinaryInvalidUtf8 { source })?,
            Term::Atom(s) if !is_sentinel_atom(s) => s.clone(),
            other => {
                return Err(ConvertError::NonStringMapKey {
                    key_kind: term_kind_str(other),
                });
            }
        };
        let value = to_value(v)?;
        obj.insert(key_str, value);
    }
    Ok(Value::Object(obj))
}

fn is_sentinel_atom(s: &str) -> bool {
    matches!(s, "nil" | "null" | "true" | "false")
}

fn term_kind_str(t: &Term) -> &'static str {
    match t {
        Term::Atom(_) => "atom",
        Term::Integer(_) => "integer",
        Term::Float(_) => "float",
        Term::Big { .. } => "big",
        Term::Binary(_) => "binary",
        Term::Tuple(_) => "tuple",
        Term::List(_) => "list",
        Term::Map(_) => "map",
    }
}

fn big_to_value(sign: bool, digits: &[u8]) -> Value {
    if digits.len() <= 8 {
        let mut magnitude: u64 = 0;
        for (i, &byte) in digits.iter().enumerate() {
            magnitude |= u64::from(byte) << (i * 8);
        }
        if !sign {
            return Value::Number(Number::from(magnitude));
        }
        if magnitude < (1u64 << 63) {
            let m_i64 = i64::try_from(magnitude).expect("magnitude < 2^63 fits in i64");
            return Value::Number(Number::from(-m_i64));
        }
        if magnitude == (1u64 << 63) {
            return Value::Number(Number::from(i64::MIN));
        }
    }
    Value::String(big_to_decimal_string(sign, digits))
}

fn big_to_decimal_string(sign: bool, digits: &[u8]) -> String {
    if digits.iter().all(|&b| b == 0) {
        return "0".to_string();
    }
    let mut work: Vec<u8> = digits.to_vec();
    let mut decimal_digits: Vec<u8> = Vec::new();
    while work.iter().any(|&b| b != 0) {
        let mut remainder: u16 = 0;
        for byte in work.iter_mut().rev() {
            let current = remainder * 256 + u16::from(*byte);
            *byte = u8::try_from(current / 10).expect("current / 10 <= 255");
            remainder = current % 10;
        }
        let digit = u8::try_from(remainder).expect("remainder < 10 fits in u8");
        decimal_digits.push(b'0' + digit);
    }
    decimal_digits.reverse();
    let mut s = String::new();
    if sign {
        s.push('-');
    }
    for &d in &decimal_digits {
        s.push(char::from(d));
    }
    s
}

fn number_to_term(n: &Number) -> Term {
    if let Some(v) = n.as_i64() {
        if let Ok(v32) = i32::try_from(v) {
            return Term::Integer(v32);
        }
        let (sign, magnitude) = if v < 0 {
            (true, v.unsigned_abs())
        } else {
            (false, u64::try_from(v).expect("v >= 0 fits in u64"))
        };
        return Term::Big {
            sign,
            digits: u64_to_big_digits(magnitude),
        };
    }
    if let Some(v) = n.as_u64() {
        return Term::Big {
            sign: false,
            digits: u64_to_big_digits(v),
        };
    }
    let v = n
        .as_f64()
        .expect("serde_json::Number::as_f64 returns Some for JSON-derived numbers");
    Term::Float(v)
}

fn u64_to_big_digits(n: u64) -> Vec<u8> {
    if n == 0 {
        return Vec::new();
    }
    let mut digits = n.to_le_bytes().to_vec();
    while digits.last() == Some(&0) {
        digits.pop();
    }
    digits
}

pub fn encode_value(value: &Value) -> Result<Vec<u8>, EncodeError> {
    let mut out = Vec::new();
    out.push(VERSION_BYTE);
    encode_value_into(value, &mut out)?;
    Ok(out)
}

pub fn encode_value_into(value: &Value, out: &mut Vec<u8>) -> Result<(), EncodeError> {
    match value {
        Value::Null => {
            write_small_atom(out, b"nil");
            Ok(())
        }
        Value::Bool(true) => {
            write_small_atom(out, b"true");
            Ok(())
        }
        Value::Bool(false) => {
            write_small_atom(out, b"false");
            Ok(())
        }
        Value::Number(n) => {
            write_number(n, out);
            Ok(())
        }
        Value::String(s) => write_binary(out, s.as_bytes()),
        Value::Array(arr) => write_list(arr, out),
        Value::Object(obj) => write_map(obj, out),
    }
}

fn write_small_atom(out: &mut Vec<u8>, bytes: &[u8]) {
    let len = u8::try_from(bytes.len()).expect("static atom byte length fits in u8");
    out.push(SMALL_ATOM_EXT);
    out.push(len);
    out.extend_from_slice(bytes);
}

fn write_number(n: &Number, out: &mut Vec<u8>) {
    if let Some(v) = n.as_i64() {
        if let Ok(v32) = i32::try_from(v) {
            if (0..=255).contains(&v32) {
                let v8 = u8::try_from(v32).expect("0..=255 fits in u8");
                out.push(SMALL_INTEGER_EXT);
                out.push(v8);
            } else {
                out.push(INTEGER_EXT);
                out.extend_from_slice(&v32.to_be_bytes());
            }
            return;
        }
        let (sign_byte, magnitude) = if v < 0 {
            (1u8, v.unsigned_abs())
        } else {
            (0u8, u64::try_from(v).expect("v >= 0 fits in u64"))
        };
        write_small_big(sign_byte, magnitude, out);
        return;
    }
    if let Some(v) = n.as_u64() {
        write_small_big(0u8, v, out);
        return;
    }
    let f = n
        .as_f64()
        .expect("serde_json::Number::as_f64 returns Some for JSON-derived numbers");
    out.push(NEW_FLOAT_EXT);
    out.extend_from_slice(&f.to_bits().to_be_bytes());
}

fn write_small_big(sign_byte: u8, magnitude: u64, out: &mut Vec<u8>) {
    let bytes = magnitude.to_le_bytes();
    let mut len: usize = 8;
    while len > 0 && bytes[len - 1] == 0 {
        len -= 1;
    }
    let len_u8 = u8::try_from(len).expect("len <= 8 fits in u8");
    out.push(SMALL_BIG_EXT);
    out.push(len_u8);
    out.push(sign_byte);
    out.extend_from_slice(&bytes[..len]);
}

fn write_binary(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), EncodeError> {
    let Ok(len_u32) = u32::try_from(bytes.len()) else {
        return Err(EncodeError::BinaryTooLong {
            byte_len: bytes.len(),
        });
    };
    out.push(BINARY_EXT);
    out.extend_from_slice(&len_u32.to_be_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

fn write_list(arr: &[Value], out: &mut Vec<u8>) -> Result<(), EncodeError> {
    if arr.is_empty() {
        out.push(NIL_EXT);
        return Ok(());
    }
    let Ok(arity_u32) = u32::try_from(arr.len()) else {
        return Err(EncodeError::ContainerTooLong {
            kind: "list",
            arity: arr.len(),
        });
    };
    out.push(LIST_EXT);
    out.extend_from_slice(&arity_u32.to_be_bytes());
    for item in arr {
        encode_value_into(item, out)?;
    }
    out.push(NIL_EXT);
    Ok(())
}

fn write_map(obj: &serde_json::Map<String, Value>, out: &mut Vec<u8>) -> Result<(), EncodeError> {
    let Ok(arity_u32) = u32::try_from(obj.len()) else {
        return Err(EncodeError::ContainerTooLong {
            kind: "map",
            arity: obj.len(),
        });
    };
    out.push(MAP_EXT);
    out.extend_from_slice(&arity_u32.to_be_bytes());
    for (k, v) in obj {
        write_binary(out, k.as_bytes())?;
        encode_value_into(v, out)?;
    }
    Ok(())
}
