#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Number, Value, json};
use ug_erlpack::{Term, decode, encode, encode_value, encode_value_into, from_value, to_value};

thread_local! {
    static COUNT_ENABLED: Cell<bool> = const { Cell::new(false) };
    static THREAD_ALLOC_COUNT: Cell<usize> = const { Cell::new(0) };
}

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNT_ENABLED.try_with(Cell::get).unwrap_or(false) {
            THREAD_ALLOC_COUNT.with(|c| c.set(c.get() + 1));
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if COUNT_ENABLED.try_with(Cell::get).unwrap_or(false) {
            THREAD_ALLOC_COUNT.with(|c| c.set(c.get() + 1));
        }
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNT_ENABLED.try_with(Cell::get).unwrap_or(false) {
            THREAD_ALLOC_COUNT.with(|c| c.set(c.get() + 1));
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOC: CountingAllocator = CountingAllocator;

fn assert_parity(v: &Value) {
    let via_term =
        encode(&from_value(v)).unwrap_or_else(|e| panic!("encode(from_value) failed: {e}"));
    let direct = encode_value(v).unwrap_or_else(|e| panic!("encode_value failed: {e}"));
    assert_eq!(
        via_term, direct,
        "encode_value diverged from encode(from_value) for {v}\nencode(from_value)={via_term:02x?}\nencode_value     ={direct:02x?}"
    );
}

#[test]
fn parity_null() {
    assert_parity(&Value::Null);
}

#[test]
fn parity_bool_true() {
    assert_parity(&Value::Bool(true));
}

#[test]
fn parity_bool_false() {
    assert_parity(&Value::Bool(false));
}

#[test]
fn parity_int_small_path() {
    for n in [0i64, 1, 7, 42, 127, 128, 254, 255] {
        assert_parity(&json!(n));
    }
}

#[test]
fn parity_int_boundary_255_256() {
    assert_parity(&json!(255i64));
    assert_parity(&json!(256i64));
}

#[test]
fn parity_int_negative_within_i32() {
    for n in [
        -1i64,
        -127,
        -128,
        -255,
        -256,
        i64::from(i32::MIN),
        i64::from(i32::MAX),
    ] {
        assert_parity(&json!(n));
    }
}

#[test]
fn parity_int_outside_i32_into_big() {
    for n in [
        i64::from(i32::MAX) + 1,
        i64::from(i32::MIN) - 1,
        1_234_567_890_123_456_789_i64,
        i64::MAX,
        i64::MIN,
    ] {
        assert_parity(&json!(n));
    }
}

#[test]
fn parity_u64_above_i64_max() {
    let v = Value::Number(Number::from(u64::MAX));
    assert_parity(&v);
    let v = Value::Number(Number::from(u64::try_from(i64::MAX).unwrap() + 1));
    assert_parity(&v);
}

#[test]
fn parity_float_finite() {
    for f in [
        0.5_f64,
        -1.5,
        std::f64::consts::PI,
        std::f64::consts::E,
        -3.14e10,
    ] {
        let v = serde_json::to_value(f).expect("finite float to_value");
        assert_parity(&v);
    }
}

#[test]
fn parity_string_variants() {
    for s in ["", "x", "hello", "café", "\u{1f389}", "discord.gg/abcd"] {
        assert_parity(&Value::String(s.to_string()));
    }
}

#[test]
fn parity_array_empty() {
    assert_parity(&Value::Array(Vec::new()));
}

#[test]
fn parity_array_simple() {
    assert_parity(&json!([1, 2, 3]));
}

#[test]
fn parity_array_nested() {
    assert_parity(&json!([[1, [2, [3, []]]], "x", null, true]));
}

#[test]
fn parity_object_empty() {
    assert_parity(&Value::Object(serde_json::Map::new()));
}

#[test]
fn parity_object_mixed() {
    assert_parity(&json!({
        "op": 0,
        "d": null,
        "s": 42,
        "t": "READY"
    }));
}

#[test]
fn parity_object_nested() {
    assert_parity(&json!({
        "user": {"id": "1234567890123456789", "username": "alice", "discriminator": "0"},
        "guilds": [
            {"id": "456", "channels": [{"id": "789", "name": "general"}]},
            {"id": "457", "channels": []}
        ],
        "extra": null,
        "flag": true,
        "ratio": 1.5,
        "big": 9_876_543_210_987_654_321_u64
    }));
}

#[test]
fn parity_object_with_inner_arrays_and_nulls() {
    assert_parity(&json!({
        "items": [
            {"k": 1},
            {"k": 2},
            {"k": 3.5, "deeper": [null, false, true, "mixed"]}
        ],
        "empty_arr": [],
        "empty_obj": {}
    }));
}

#[test]
fn encode_value_into_zero_alloc_with_presized_buffer() {
    let payload = json!({
        "op": 0,
        "d": {
            "v": 9,
            "user": {"id": "1234567890123456789", "username": "alice", "bot": false},
            "guilds": [
                {"id": "111", "name": "G1", "channels": [
                    {"id": "211", "name": "general", "type": 0}
                ]},
                {"id": "222", "name": "G2", "channels": []}
            ],
            "presences": [],
            "_trace": ["underground-gateway"]
        },
        "s": 1,
        "t": "READY"
    });

    let probe = encode_value(&payload).expect("probe encode_value");
    let mut out: Vec<u8> = Vec::with_capacity(probe.len() * 4);

    COUNT_ENABLED.with(|c| c.set(false));
    THREAD_ALLOC_COUNT.with(|c| c.set(0));

    COUNT_ENABLED.with(|c| c.set(true));
    let res = encode_value_into(&payload, &mut out);
    COUNT_ENABLED.with(|c| c.set(false));

    let count = THREAD_ALLOC_COUNT.with(Cell::get);
    res.expect("encode_value_into");

    assert_eq!(
        count, 0,
        "encode_value_into allocated {count} times beyond the pre-sized output buffer"
    );
    assert_eq!(
        &out[..],
        &probe[1..],
        "encode_value_into bytes differ from encode_value tail"
    );
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn fixture_value_roundtrip(name: &str) {
    let path = fixtures_dir().join(format!("{name}.etf"));
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let (term, consumed) = decode(&bytes).unwrap_or_else(|e| panic!("decode({name}): {e}"));
    assert_eq!(
        consumed,
        bytes.len(),
        "decode({name}) did not consume all bytes"
    );
    let value = to_value(&term).unwrap_or_else(|e| panic!("to_value({name}): {e}"));
    let via_term = encode(&from_value(&value)).unwrap_or_else(|e| panic!("encode({name}): {e}"));
    let direct = encode_value(&value).unwrap_or_else(|e| panic!("encode_value({name}): {e}"));
    assert_eq!(
        via_term, direct,
        "fixture {name}: encode_value diverged from encode(from_value)"
    );
}

#[test]
fn fixture_parity_simple_map() {
    fixture_value_roundtrip("simple_map");
}

#[test]
fn fixture_parity_nested_list() {
    fixture_value_roundtrip("nested_list");
}

#[test]
fn fixture_parity_binary_hello() {
    fixture_value_roundtrip("binary_hello");
}

#[test]
fn fixture_parity_binary_utf8() {
    fixture_value_roundtrip("binary_utf8");
}

#[test]
fn fixture_parity_snowflake() {
    fixture_value_roundtrip("snowflake_8bytes");
}

#[test]
fn fixture_parity_small_ints() {
    for name in [
        "small_int_0",
        "small_int_255",
        "int_256",
        "int_neg1",
        "int_max_i32",
    ] {
        fixture_value_roundtrip(name);
    }
}

#[test]
fn fixture_parity_atoms_become_strings_then_reencoded_as_binary() {
    let bytes = fs::read(fixtures_dir().join("nil_atom.etf")).expect("read nil_atom");
    let (term, _) = decode(&bytes).expect("decode nil_atom");
    assert_eq!(term, Term::Atom("nil".into()));
    let value = to_value(&term).expect("to_value");
    assert_eq!(value, Value::Null);
    assert_parity(&value);
}

#[test]
fn encode_value_envelope_matches_manual_term_path() {
    let inner = json!({
        "id": "1234567890123456789",
        "channel_id": "9876543210987654321",
        "content": "hello world",
        "embeds": [],
        "attachments": [],
        "mentions": []
    });
    let envelope = json!({
        "op": 0,
        "d": inner,
        "s": 42,
        "t": "MESSAGE_CREATE"
    });
    let via_term = encode(&from_value(&envelope)).expect("encode(from_value)");
    let direct = encode_value(&envelope).expect("encode_value");
    assert_eq!(via_term, direct);
}
