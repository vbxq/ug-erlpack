#![cfg(feature = "splice")]

use std::fs;
use std::path::{Path, PathBuf};

use ug_erlpack::{SplicedValue, Term, decode, encode, splice_map};

fn corpus() -> Vec<Vec<(Term, Term)>> {
    vec![
        vec![],
        vec![(Term::Binary(b"op".to_vec()), Term::Integer(10))],
        vec![
            (Term::Binary(b"op".to_vec()), Term::Integer(10)),
            (Term::Atom("ok".into()), Term::Atom("nil".into())),
        ],
        vec![
            (
                Term::Binary(b"a".to_vec()),
                Term::Map(vec![(Term::Binary(b"x".to_vec()), Term::Integer(1))]),
            ),
            (
                Term::Binary(b"b".to_vec()),
                Term::List(vec![Term::Integer(1), Term::Integer(2)]),
            ),
        ],
        vec![
            (
                Term::Binary(b"id".to_vec()),
                Term::Big {
                    sign: false,
                    digits: 1_234_567_890_123_456_789_u64.to_le_bytes().to_vec(),
                },
            ),
            (
                Term::Binary(b"pi".to_vec()),
                Term::Float(std::f64::consts::PI),
            ),
        ],
        vec![(
            Term::Atom("status".into()),
            Term::Binary(b"online".to_vec()),
        )],
        vec![
            (
                Term::Binary(b"name".to_vec()),
                Term::Binary(b"test".to_vec()),
            ),
            (Term::Binary(b"count".to_vec()), Term::Integer(42)),
            (Term::Binary(b"ok".to_vec()), Term::Atom("true".into())),
        ],
        vec![
            (Term::Binary(b"a".to_vec()), Term::Integer(1)),
            (Term::Binary(b"b".to_vec()), Term::Integer(2)),
            (Term::Binary(b"c".to_vec()), Term::Integer(3)),
            (Term::Binary(b"d".to_vec()), Term::Integer(-1)),
            (Term::Binary(b"e".to_vec()), Term::Integer(256)),
        ],
    ]
}

fn extract_value_bytes(key: &Term, value: &Term) -> Vec<u8> {
    let b_solo = encode(&Term::Map(vec![(key.clone(), value.clone())]))
        .expect("encode single-entry map (real value)");
    let probe = encode(&Term::Map(vec![(key.clone(), Term::Integer(0))]))
        .expect("encode single-entry map (probe value)");
    assert!(
        probe.len() >= 8,
        "probe length {} is below the 6-byte map prefix + 2-byte small_int suffix floor",
        probe.len()
    );
    let key_len = probe.len() - 8;
    assert!(
        b_solo.len() >= 6 + key_len,
        "b_solo length {} cannot fit the 6-byte map prefix + {key_len}-byte key",
        b_solo.len()
    );
    b_solo[6 + key_len..].to_vec()
}

#[test]
fn owned_only_baseline() {
    for (idx, shape) in corpus().iter().enumerate() {
        let baseline =
            encode(&Term::Map(shape.clone())).expect("encode baseline map should succeed");
        let spliced_entries: Vec<(Term, SplicedValue<'_>)> = shape
            .iter()
            .map(|(k, v)| (k.clone(), SplicedValue::Owned(v.clone())))
            .collect();
        let spliced = splice_map(&spliced_entries).expect("splice_map all-Owned should succeed");
        assert_eq!(
            baseline, spliced,
            "shape {idx}: baseline encode vs all-Owned splice_map must be byte-identical"
        );
    }
}

#[test]
fn mixed_owned_preencoded_equivalence() {
    for (shape_idx, shape) in corpus().iter().enumerate() {
        let baseline =
            encode(&Term::Map(shape.clone())).expect("encode baseline map should succeed");
        for n in 0..shape.len() {
            let (key_n, value_n) = &shape[n];
            let v_bytes = extract_value_bytes(key_n, value_n);
            let mixed: Vec<(Term, SplicedValue<'_>)> = shape
                .iter()
                .enumerate()
                .map(|(i, (k, v))| {
                    let value = if i == n {
                        SplicedValue::PreEncoded(&v_bytes)
                    } else {
                        SplicedValue::Owned(v.clone())
                    };
                    (k.clone(), value)
                })
                .collect();
            let spliced =
                splice_map(&mixed).expect("splice_map with one PreEncoded slot should succeed");
            assert_eq!(
                baseline, spliced,
                "shape {shape_idx} position {n}: \
                 splice_map with PreEncoded slot must match all-Owned baseline byte-for-byte"
            );
        }
    }
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

#[test]
fn splice_into_upstream_fixtures() {
    let dir = fixtures_dir();
    let mut top_level_maps = 0;
    for entry in fs::read_dir(&dir).expect("read fixtures dir") {
        let entry = entry.expect("read fixtures entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("etf") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("fixture file stem")
            .to_string();
        let bytes = fs::read(&path).expect("read fixture bytes");
        let (term, consumed) = decode(&bytes).unwrap_or_else(|e| {
            panic!("decode fixture {name}: {e} (kind={})", e.kind_str());
        });
        assert_eq!(
            consumed,
            bytes.len(),
            "fixture {name}: decode consumed {consumed} of {} bytes",
            bytes.len()
        );
        let Term::Map(entries) = term else {
            continue;
        };
        top_level_maps += 1;
        for (idx, (key, value)) in entries.iter().enumerate() {
            let v_bytes = extract_value_bytes(key, value);
            let wrapper = splice_map(&[(
                Term::Binary(b"wrapper".to_vec()),
                SplicedValue::PreEncoded(&v_bytes),
            )])
            .expect("splice_map wrapper");
            let (wrapper_term, wrapper_consumed) = decode(&wrapper).unwrap_or_else(|e| {
                panic!(
                    "decode wrapper from fixture {name} entry {idx}: {e} (kind={})",
                    e.kind_str()
                );
            });
            assert_eq!(
                wrapper_consumed,
                wrapper.len(),
                "fixture {name} entry {idx}: wrapper decode consumed {wrapper_consumed} of {} bytes",
                wrapper.len()
            );
            let Term::Map(wrapper_entries) = wrapper_term else {
                panic!("fixture {name} entry {idx}: wrapper top-level was not a Map");
            };
            assert_eq!(
                wrapper_entries.len(),
                1,
                "fixture {name} entry {idx}: wrapper map arity was {}, expected 1",
                wrapper_entries.len()
            );
            let (wrapper_key, wrapper_value) = &wrapper_entries[0];
            assert_eq!(
                wrapper_key,
                &Term::Binary(b"wrapper".to_vec()),
                "fixture {name} entry {idx}: wrapper key did not round-trip"
            );
            assert_eq!(
                wrapper_value, value,
                "fixture {name} entry {idx}: spliced value did not round-trip to original decoded value"
            );
        }
    }
    assert!(
        top_level_maps >= 1,
        "expected at least one top-level map fixture in {}",
        dir.display()
    );
}
