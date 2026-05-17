use ug_erlpack::{Term, decode, encode};

struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    fn next_u8(&mut self) -> u8 {
        (self.next_u64() >> 56) as u8
    }

    fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    fn next_i32(&mut self) -> i32 {
        i32::from_ne_bytes(self.next_u32().to_ne_bytes())
    }

    fn next_bool(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }

    #[allow(clippy::cast_possible_truncation)]
    fn range(&mut self, lo: usize, hi: usize) -> usize {
        let span = hi - lo;
        lo + (self.next_u64() as usize) % span
    }
}

fn check_roundtrip(term: &Term) {
    let encoded =
        encode(term).unwrap_or_else(|e| panic!("encode failed: {e} (kind={})", e.kind_str()));
    let (decoded, consumed) =
        decode(&encoded).unwrap_or_else(|e| panic!("decode failed: {e} (kind={})", e.kind_str()));
    assert_eq!(
        consumed,
        encoded.len(),
        "decode consumed {consumed} of {} bytes",
        encoded.len()
    );
    assert_eq!(&decoded, term, "roundtrip diverged: encoded={encoded:02x?}",);
}

#[test]
fn roundtrip_atom_variants() {
    for s in [
        "nil",
        "true",
        "false",
        "null",
        "x",
        "ASCII-ish_atom",
        "", // empty atom
    ] {
        check_roundtrip(&Term::Atom(s.to_string()));
    }
}

#[test]
fn roundtrip_integer_small_path() {
    for n in [0, 1, 7, 42, 127, 128, 254, 255] {
        check_roundtrip(&Term::Integer(n));
    }
}

#[test]
fn roundtrip_integer_boundary_255_to_256() {
    check_roundtrip(&Term::Integer(255));
    check_roundtrip(&Term::Integer(256));
}

#[test]
fn roundtrip_integer_signed_extremes() {
    for n in [-1, -255, -256, i32::MIN, i32::MAX] {
        check_roundtrip(&Term::Integer(n));
    }
}

#[test]
fn roundtrip_float_finite_values() {
    for f in [
        0.0_f64,
        -0.0,
        1.0,
        -1.0,
        std::f64::consts::PI,
        std::f64::consts::E,
        f64::MIN,
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::EPSILON,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        check_roundtrip(&Term::Float(f));
    }
}

#[test]
fn roundtrip_big_zero_digits() {
    check_roundtrip(&Term::Big {
        sign: false,
        digits: vec![],
    });
    check_roundtrip(&Term::Big {
        sign: true,
        digits: vec![],
    });
}

#[test]
fn roundtrip_big_8_digits_snowflake() {
    let id: u64 = 1_234_567_890_123_456_789;
    check_roundtrip(&Term::Big {
        sign: false,
        digits: id.to_le_bytes().to_vec(),
    });
    check_roundtrip(&Term::Big {
        sign: true,
        digits: id.to_le_bytes().to_vec(),
    });
}

#[test]
fn roundtrip_big_1_to_8_digits() {
    for n in 1u8..=8 {
        let digits: Vec<u8> = (0..n).map(|i| i.wrapping_add(1).wrapping_mul(17)).collect();
        check_roundtrip(&Term::Big {
            sign: false,
            digits: digits.clone(),
        });
        check_roundtrip(&Term::Big { sign: true, digits });
    }
}

#[test]
fn roundtrip_binary_variants() {
    for bytes in [
        Vec::new(),
        vec![0],
        b"hello".to_vec(),
        "café \u{1f389}".as_bytes().to_vec(),
        (0..=255u8).collect::<Vec<u8>>(),
    ] {
        check_roundtrip(&Term::Binary(bytes));
    }
}

#[test]
fn roundtrip_atom_max_length_254() {
    let s = "a".repeat(254);
    check_roundtrip(&Term::Atom(s));
}

#[test]
fn roundtrip_empty_containers() {
    check_roundtrip(&Term::Tuple(vec![]));
    check_roundtrip(&Term::List(vec![]));
    check_roundtrip(&Term::Map(vec![]));
}

#[test]
fn roundtrip_mixed_map_keys() {
    let m = Term::Map(vec![
        (Term::Binary(b"op".to_vec()), Term::Integer(10)),
        (Term::Atom("ok".into()), Term::Atom("nil".into())),
        (
            Term::Binary(b"d".to_vec()),
            Term::List(vec![Term::Integer(1), Term::Integer(2)]),
        ),
    ]);
    check_roundtrip(&m);
}

#[test]
fn roundtrip_nested_depth_4() {
    let t = Term::List(vec![
        Term::Tuple(vec![
            Term::Integer(1),
            Term::Map(vec![(
                Term::Binary(b"k".to_vec()),
                Term::List(vec![Term::Atom("nil".into()), Term::Integer(42)]),
            )]),
        ]),
        Term::Map(vec![
            (Term::Binary(b"a".to_vec()), Term::Integer(7)),
            (
                Term::Binary(b"b".to_vec()),
                Term::Tuple(vec![Term::Binary(b"nested".to_vec())]),
            ),
        ]),
    ]);
    check_roundtrip(&t);
}

#[test]
fn roundtrip_tuple_small_to_large_boundary() {
    let small: Vec<Term> = (0i32..=255).map(Term::Integer).collect();
    check_roundtrip(&Term::Tuple(small));
    let large: Vec<Term> = (0i32..=256).map(Term::Integer).collect();
    check_roundtrip(&Term::Tuple(large));
}

#[test]
fn roundtrip_string_ext_decodes_as_integer_list() {
    let mut bytes = vec![131, 107, 0, 3];
    bytes.extend_from_slice(b"abc");
    let (term, consumed) = decode(&bytes).unwrap();
    assert_eq!(consumed, bytes.len());
    assert_eq!(
        term,
        Term::List(vec![
            Term::Integer(i32::from(b'a')),
            Term::Integer(i32::from(b'b')),
            Term::Integer(i32::from(b'c')),
        ]),
    );
}

fn gen_atom_string(rng: &mut Lcg) -> String {
    let n = rng.range(0, 33);
    let mut s = String::with_capacity(n);
    for _ in 0..n {
        let c = match rng.range(0, 4) {
            0 => char::from(b'a' + (rng.next_u8() % 26)),
            1 => char::from(b'A' + (rng.next_u8() % 26)),
            2 => char::from(b'0' + (rng.next_u8() % 10)),
            _ => '_',
        };
        s.push(c);
    }
    s
}

fn gen_integer(rng: &mut Lcg) -> i32 {
    match rng.range(0, 6) {
        0 => 0,
        1 => 255,
        2 => 256,
        3 => -1,
        4 => i32::from(rng.next_u8()),
        _ => rng.next_i32(),
    }
}

fn gen_float(rng: &mut Lcg) -> f64 {
    const POOL: [f64; 12] = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        std::f64::consts::PI,
        std::f64::consts::E,
        42.5,
        -3.14e10,
        f64::MIN,
        f64::MAX,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    POOL[rng.range(0, POOL.len())]
}

fn gen_big_digits(rng: &mut Lcg) -> Vec<u8> {
    let n = rng.range(0, 9);
    (0..n).map(|_| rng.next_u8()).collect()
}

fn gen_binary(rng: &mut Lcg) -> Vec<u8> {
    let n = rng.range(0, 32);
    (0..n).map(|_| rng.next_u8()).collect()
}

fn gen_term(rng: &mut Lcg, depth: u32) -> Term {
    let upper = if depth == 0 { 6 } else { 9 };
    match rng.range(0, upper) {
        0 => Term::Atom(gen_atom_string(rng)),
        1 => Term::Integer(gen_integer(rng)),
        2 => Term::Float(gen_float(rng)),
        3 => Term::Big {
            sign: rng.next_bool(),
            digits: gen_big_digits(rng),
        },
        4 => Term::Binary(gen_binary(rng)),
        5 => Term::List(vec![]),
        6 => Term::Tuple(gen_seq(rng, depth)),
        7 => Term::List(gen_seq(rng, depth)),
        _ => Term::Map(gen_map(rng, depth)),
    }
}

fn gen_seq(rng: &mut Lcg, depth: u32) -> Vec<Term> {
    let n = rng.range(0, 6);
    (0..n).map(|_| gen_term(rng, depth - 1)).collect()
}

fn gen_map(rng: &mut Lcg, depth: u32) -> Vec<(Term, Term)> {
    let n = rng.range(0, 5);
    (0..n)
        .map(|_| {
            let key = match rng.range(0, 2) {
                0 => Term::Binary(gen_binary(rng)),
                _ => Term::Atom(gen_atom_string(rng)),
            };
            let value = gen_term(rng, depth - 1);
            (key, value)
        })
        .collect()
}

#[test]
fn roundtrip_random_terms_depth_4() {
    let mut rng = Lcg::new(0x00C0_FFEE_BABE);
    for i in 0..64 {
        let t = gen_term(&mut rng, 4);
        let encoded = encode(&t)
            .unwrap_or_else(|e| panic!("iter {i}: encode failed: {e} (kind={})", e.kind_str()));
        let (decoded, consumed) = decode(&encoded)
            .unwrap_or_else(|e| panic!("iter {i}: decode failed: {e} (kind={})", e.kind_str()));
        assert_eq!(consumed, encoded.len(), "iter {i}: consumed != len");
        assert_eq!(decoded, t, "iter {i}: roundtrip diverged");
    }
}

#[test]
fn roundtrip_random_terms_shallow() {
    let mut rng = Lcg::new(0xDEAD_BEEF);
    for i in 0..64 {
        let t = gen_term(&mut rng, 1);
        let encoded = encode(&t).unwrap_or_else(|e| panic!("iter {i}: {e}"));
        let (decoded, _) = decode(&encoded).unwrap_or_else(|e| panic!("iter {i}: {e}"));
        assert_eq!(decoded, t, "iter {i}: roundtrip diverged");
    }
}
