use std::fs;
use std::path::{Path, PathBuf};

use ug_erlpack::{Term, decode};

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn load(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(format!("{name}.etf"));
    fs::read(&path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

fn assert_decodes_to(name: &str, expected: &Term) {
    let bytes = load(name);
    let (term, consumed) = decode(&bytes)
        .unwrap_or_else(|e| panic!("decode({name}) failed: {e} (kind={})", e.kind_str()));
    assert_eq!(
        consumed,
        bytes.len(),
        "decode({name}) consumed {consumed} of {} bytes",
        bytes.len()
    );
    assert_eq!(&term, expected, "decode({name}) produced unexpected Term");
}

#[test]
fn nil_atom() {
    assert_decodes_to("nil_atom", &Term::Atom("nil".into()));
}

#[test]
fn true_atom() {
    assert_decodes_to("true_atom", &Term::Atom("true".into()));
}

#[test]
fn false_atom() {
    assert_decodes_to("false_atom", &Term::Atom("false".into()));
}

#[test]
fn small_int_0() {
    assert_decodes_to("small_int_0", &Term::Integer(0));
}

#[test]
fn small_int_255() {
    assert_decodes_to("small_int_255", &Term::Integer(255));
}

#[test]
fn int_256() {
    assert_decodes_to("int_256", &Term::Integer(256));
}

#[test]
fn int_neg1() {
    assert_decodes_to("int_neg1", &Term::Integer(-1));
}

#[test]
fn int_max_i32() {
    assert_decodes_to("int_max_i32", &Term::Integer(i32::MAX));
}

#[test]
fn big_2to33() {
    assert_decodes_to(
        "big_2to33",
        &Term::Big {
            sign: false,
            digits: vec![0, 0, 0, 0, 2],
        },
    );
}

#[test]
fn snowflake_8bytes() {
    let id: u64 = 1_234_567_890_123_456_789;
    assert_decodes_to(
        "snowflake_8bytes",
        &Term::Big {
            sign: false,
            digits: id.to_le_bytes().to_vec(),
        },
    );
}

#[test]
fn binary_hello() {
    assert_decodes_to("binary_hello", &Term::Binary(b"hello".to_vec()));
}

#[test]
fn binary_utf8() {
    assert_decodes_to(
        "binary_utf8",
        &Term::Binary("café \u{1f389}".as_bytes().to_vec()),
    );
}

#[test]
fn empty_list() {
    let bytes = load("empty_list");
    assert_eq!(bytes, vec![131, 106], "empty list wire shape");
    assert_decodes_to("empty_list", &Term::List(vec![]));
}

#[test]
fn nested_list() {
    assert_decodes_to(
        "nested_list",
        &Term::List(vec![
            Term::Integer(1),
            Term::List(vec![Term::Integer(2), Term::Integer(3)]),
            Term::Integer(4),
        ]),
    );
}

#[test]
fn simple_map() {
    assert_decodes_to(
        "simple_map",
        &Term::Map(vec![
            (Term::Binary(b"op".to_vec()), Term::Integer(10)),
            (Term::Binary(b"d".to_vec()), Term::Atom("nil".into())),
        ]),
    );
}

#[test]
fn tuple_2() {
    assert_decodes_to(
        "tuple_2",
        &Term::Tuple(vec![Term::Integer(1), Term::Integer(2)]),
    );
}

#[test]
fn encoder_matches_upstream_byte_for_byte() {
    let cases = [
        "nil_atom",
        "true_atom",
        "false_atom",
        "small_int_0",
        "small_int_255",
        "int_256",
        "int_neg1",
        "int_max_i32",
        "big_2to33",
        "snowflake_8bytes",
        "binary_hello",
        "binary_utf8",
        "empty_list",
        "nested_list",
        "simple_map",
        "tuple_2",
    ];
    for name in cases {
        let bytes = load(name);
        let (term, _) = decode(&bytes).unwrap_or_else(|e| panic!("decode {name}: {e}"));
        let reencoded = ug_erlpack::encode(&term).unwrap_or_else(|e| panic!("encode {name}: {e}"));
        assert_eq!(
            reencoded, bytes,
            "{name}: re-encoded bytes differ from upstream"
        );
    }
}
