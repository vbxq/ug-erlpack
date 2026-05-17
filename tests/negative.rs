use ug_erlpack::{DecodeConfig, DecodeError, decode, decode_with};

#[test]
fn bad_version_byte() {
    let bytes = [42u8];
    let err = decode(&bytes).expect_err("should reject bad version");
    assert_eq!(err.kind_str(), "bad_version");
    match err {
        DecodeError::BadVersion { found } => assert_eq!(found, 42),
        other => panic!("expected BadVersion, got {other:?}"),
    }
}

#[test]
fn unknown_tag() {
    let bytes = [131u8, 0];
    let err = decode(&bytes).expect_err("should reject unknown tag");
    assert_eq!(err.kind_str(), "unknown_tag");
    match err {
        DecodeError::UnknownTag { tag, offset } => {
            assert_eq!(tag, 0);
            assert_eq!(offset, 1, "tag byte position");
        }
        other => panic!("expected UnknownTag, got {other:?}"),
    }
}

#[test]
fn compressed_not_supported() {
    let bytes = [131u8, 80];
    let err = decode(&bytes).expect_err("should reject COMPRESSED");
    assert_eq!(err.kind_str(), "compressed_not_supported");
    match err {
        DecodeError::CompressedNotSupported { offset } => {
            assert_eq!(offset, 1);
        }
        other => panic!("expected CompressedNotSupported, got {other:?}"),
    }
}

#[test]
fn bit_binary_not_supported() {
    let bytes = [131u8, 77];
    let err = decode(&bytes).expect_err("should reject BIT_BINARY_EXT");
    assert_eq!(err.kind_str(), "bit_binary_not_supported");
    match err {
        DecodeError::BitBinaryNotSupported { offset } => {
            assert_eq!(offset, 1);
        }
        other => panic!("expected BitBinaryNotSupported, got {other:?}"),
    }
}

#[test]
fn erlang_runtime_term_not_supported_all_tags() {
    for &tag in &[101u8, 102, 103, 112, 113, 114, 117] {
        let bytes = [131u8, tag];
        let err =
            decode(&bytes).expect_err(&format!("tag {tag} should be refused as runtime term"));
        assert_eq!(
            err.kind_str(),
            "erlang_runtime_term_not_supported",
            "tag {tag} should map to ErlangRuntimeTermNotSupported",
        );
        if let DecodeError::ErlangRuntimeTermNotSupported {
            tag: got_tag,
            offset,
        } = err
        {
            assert_eq!(got_tag, tag);
            assert_eq!(offset, 1);
        } else {
            panic!("tag {tag}: wrong DecodeError variant");
        }
    }
}

#[test]
fn unexpected_eof_empty_input() {
    let bytes: [u8; 0] = [];
    let err = decode(&bytes).expect_err("should fail on empty input");
    assert_eq!(err.kind_str(), "unexpected_eof");
    match err {
        DecodeError::UnexpectedEof {
            needed,
            available,
            offset,
        } => {
            assert_eq!(needed, 1);
            assert_eq!(available, 0);
            assert_eq!(offset, 0);
        }
        other => panic!("expected UnexpectedEof, got {other:?}"),
    }
}

#[test]
fn unexpected_eof_mid_integer() {
    let bytes = [131u8, 97];
    let err = decode(&bytes).expect_err("should fail mid-integer");
    assert_eq!(err.kind_str(), "unexpected_eof");
    match err {
        DecodeError::UnexpectedEof {
            needed,
            available,
            offset,
        } => {
            assert_eq!(needed, 1);
            assert_eq!(available, 0);
            assert_eq!(offset, 2);
        }
        other => panic!("expected UnexpectedEof, got {other:?}"),
    }
}

#[test]
fn big_digit_ceiling_exceeded_large_big() {
    let cfg = DecodeConfig::default().with_big_digit_ceiling(8);
    let bytes = [131u8, 111, 0, 0, 0, 64];
    let err = decode_with(&bytes, &cfg).expect_err("should reject oversized big");
    assert_eq!(err.kind_str(), "big_digit_ceiling_exceeded");
    match err {
        DecodeError::BigDigitCeilingExceeded {
            declared,
            ceiling,
            offset,
        } => {
            assert_eq!(declared, 64);
            assert_eq!(ceiling, 8);
            assert_eq!(offset, 1, "offset of tag byte (111)");
        }
        other => panic!("expected BigDigitCeilingExceeded, got {other:?}"),
    }
}

#[test]
fn big_digit_ceiling_exceeded_small_big() {
    let cfg = DecodeConfig::default().with_big_digit_ceiling(8);
    let bytes = [131u8, 110, 10];
    let err = decode_with(&bytes, &cfg).expect_err("should reject oversized small-big");
    assert_eq!(err.kind_str(), "big_digit_ceiling_exceeded");
    match err {
        DecodeError::BigDigitCeilingExceeded {
            declared,
            ceiling,
            offset,
        } => {
            assert_eq!(declared, 10);
            assert_eq!(ceiling, 8);
            assert_eq!(offset, 1);
        }
        other => panic!("expected BigDigitCeilingExceeded, got {other:?}"),
    }
}

#[test]
fn big_digit_ceiling_before_eof_check() {
    let cfg = DecodeConfig::default().with_big_digit_ceiling(8);
    let bytes = [131u8, 111, 0, 0, 0, 64];
    let err = decode_with(&bytes, &cfg).expect_err("should reject by ceiling first");
    assert_eq!(
        err.kind_str(),
        "big_digit_ceiling_exceeded",
        "ceiling check must precede EOF check"
    );
}

#[test]
fn improper_list_non_nil_tail() {
    let bytes = [131u8, 108, 0, 0, 0, 1, 97, 1, 97, 2];
    let err = decode(&bytes).expect_err("should reject improper list");
    assert_eq!(err.kind_str(), "improper_list");
    match err {
        DecodeError::ImproperList { tail_tag, offset } => {
            assert_eq!(tail_tag, 97);
            // offset = version(1) + tag(1) + arity(4) + element(2) = 8.
            assert_eq!(offset, 8);
        }
        other => panic!("expected ImproperList, got {other:?}"),
    }
}

#[test]
fn atom_invalid_utf8_atom_utf8_ext() {
    let bytes = [131u8, 118, 0, 1, 0x80];
    let err = decode(&bytes).expect_err("should reject invalid UTF-8 atom");
    assert_eq!(err.kind_str(), "atom_invalid_utf8");
    match err {
        DecodeError::AtomInvalidUtf8 { offset, source: _ } => {
            // payload_offset = version(1) + tag(1) + length(2) = 4.
            assert_eq!(offset, 4);
        }
        other => panic!("expected AtomInvalidUtf8, got {other:?}"),
    }
}

#[test]
fn atom_invalid_utf8_small_atom_utf8_ext() {
    let bytes = [131u8, 119, 2, 0xC3, 0x28];
    let err = decode(&bytes).expect_err("should reject invalid UTF-8 small atom");
    assert_eq!(err.kind_str(), "atom_invalid_utf8");
    match err {
        DecodeError::AtomInvalidUtf8 { offset, .. } => {
            // payload_offset = version(1) + tag(1) + length(1) = 3.
            assert_eq!(offset, 3);
        }
        other => panic!("expected AtomInvalidUtf8, got {other:?}"),
    }
}

#[test]
fn legacy_float_parse_failed() {
    let mut bytes = vec![131u8, 99];
    bytes.extend(std::iter::repeat_n(b'X', 31));
    let err = decode(&bytes).expect_err("should reject unparseable legacy float");
    assert_eq!(err.kind_str(), "legacy_float_parse_failed");
    match err {
        DecodeError::LegacyFloatParseFailed { offset } => {
            assert_eq!(offset, 1, "tag-byte offset");
        }
        other => panic!("expected LegacyFloatParseFailed, got {other:?}"),
    }
}

#[test]
fn binary_truncated_payload() {
    let bytes = [131u8, 109, 0, 0, 0, 10, 1, 2, 3];
    let err = decode(&bytes).expect_err("should fail on truncated binary");
    assert_eq!(err.kind_str(), "unexpected_eof");
    match err {
        DecodeError::UnexpectedEof {
            needed,
            available,
            offset,
        } => {
            assert_eq!(needed, 10);
            assert_eq!(available, 3);
            assert_eq!(offset, 6);
        }
        other => panic!("expected UnexpectedEof, got {other:?}"),
    }
}

#[test]
fn trailing_bytes_not_an_error() {
    let bytes = [131u8, 97, 42, 0xff, 0xee];
    let (term, consumed) = decode(&bytes).expect("decode should succeed");
    assert_eq!(term, ug_erlpack::Term::Integer(42));
    assert_eq!(consumed, 3, "consumed = 1 version + 2 small-integer");
}
