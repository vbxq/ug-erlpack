use ug_erlpack::{DecodeConfig, DecodeError, Term, decode, decode_with};

fn nested_tuples(depth: usize) -> Vec<u8> {
    let mut buf = vec![131u8];
    for _ in 0..depth {
        buf.push(104);
        buf.push(1);
    }
    buf.push(97);
    buf.push(0);
    buf
}

fn nested_lists(depth: usize) -> Vec<u8> {
    let mut buf = vec![131u8];
    for _ in 0..depth {
        buf.push(108);
        buf.extend_from_slice(&1u32.to_be_bytes());
    }
    buf.push(97);
    buf.push(0);
    buf.extend(std::iter::repeat_n(106u8, depth));
    buf
}

#[test]
fn decodes_tuples_exactly_at_max_depth() {
    let cfg = DecodeConfig::default().with_max_depth(4);
    let payload = nested_tuples(4);
    let (term, consumed) =
        decode_with(&payload, &cfg).expect("leaf at depth 4 must decode with max_depth 4");
    assert_eq!(consumed, payload.len());
    let mut cur = &term;
    for _ in 0..4 {
        match cur {
            Term::Tuple(elems) => {
                assert_eq!(elems.len(), 1);
                cur = &elems[0];
            }
            other => panic!("expected Tuple, got {other:?}"),
        }
    }
    assert_eq!(*cur, Term::Integer(0));
}

#[test]
fn rejects_tuples_over_max_depth() {
    let cfg = DecodeConfig::default().with_max_depth(4);
    let err = decode_with(&nested_tuples(5), &cfg)
        .expect_err("leaf at depth 5 must be refused with max_depth 4");
    assert!(
        matches!(
            err,
            DecodeError::MaxDepthExceeded {
                depth: 5,
                ceiling: 4,
                ..
            }
        ),
        "got {err:?}"
    );
}

#[test]
fn rejects_lists_over_max_depth() {
    let cfg = DecodeConfig::default().with_max_depth(4);
    let err = decode_with(&nested_lists(5), &cfg)
        .expect_err("list depth 5 must be refused with max_depth 4");
    assert!(
        matches!(err, DecodeError::MaxDepthExceeded { .. }),
        "got {err:?}"
    );
}

#[test]
fn default_config_refuses_deeply_nested_dos_payload() {
    let err = decode(&nested_tuples(1000))
        .expect_err("deeply nested payload must be refused, not decoded or crashed");
    assert!(
        matches!(err, DecodeError::MaxDepthExceeded { .. }),
        "got {err:?}"
    );
    assert_eq!(err.kind_str(), "max_depth_exceeded");
}
