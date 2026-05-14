#!/usr/bin/env python3
import os
import sys

try:
    import erlpack
except ImportError:
    sys.stderr.write(
        "erlpack not installed. Install via:\n"
        "    python3 -m venv /tmp/erlpack-venv\n"
        "    /tmp/erlpack-venv/bin/pip install erlpack\n"
    )
    sys.exit(1)

FIXTURES_DIR = os.path.dirname(os.path.abspath(__file__))

# (filename_base, value, description)
CASES = [
    ("nil_atom", None, "Term::Atom(\"nil\")"),
    ("true_atom", True, "Term::Atom(\"true\")"),
    ("false_atom", False, "Term::Atom(\"false\")"),
    ("small_int_0", 0, "Term::Integer(0)  // SMALL_INTEGER_EXT path"),
    ("small_int_255", 255, "Term::Integer(255)  // SMALL_INTEGER_EXT upper bound"),
    ("int_256", 256, "Term::Integer(256)  // INTEGER_EXT (one past SMALL_INTEGER_EXT)"),
    ("int_neg1", -1, "Term::Integer(-1)  // INTEGER_EXT (negative)"),
    ("int_max_i32", 2**31 - 1, "Term::Integer(i32::MAX)  // INTEGER_EXT upper bound"),
    ("big_2to33", 2**33, "Term::Big { sign: false, digits: vec![0, 0, 0, 0, 2] }  // SMALL_BIG_EXT n=5"),
    (
        "snowflake_8bytes",
        1234567890123456789,
        "Term::Big { sign: false, digits: 1234567890123456789u64.to_le_bytes().to_vec() }  // SMALL_BIG_EXT n=8 (Discord snowflake canonical case)",
    ),
    ("binary_hello", "hello", "Term::Binary(b\"hello\".to_vec())  // BINARY_EXT, UTF-8 ASCII"),
    (
        "binary_utf8",
        "café \U0001f389",
        "Term::Binary(\"caf\\u{e9} \\u{1f389}\".as_bytes().to_vec())  // BINARY_EXT, multibyte UTF-8",
    ),
    ("empty_list", [], "Term::List(vec![])  // bare NIL_EXT byte"),
    (
        "nested_list",
        [1, [2, 3], 4],
        "Term::List(vec![Integer(1), List(vec![Integer(2), Integer(3)]), Integer(4)])",
    ),
    (
        "simple_map",
        {"op": 10, "d": None},
        "Term::Map(vec![(Binary(b\"op\"), Integer(10)), (Binary(b\"d\"), Atom(\"nil\"))])  // OP 10 HELLO shape (illustrative)",
    ),
    (
        "tuple_2",
        (1, 2),
        "Term::Tuple(vec![Integer(1), Integer(2)])  // SMALL_TUPLE_EXT arity=2",
    ),
]


def main() -> int:
    for name, value, description in CASES:
        bytes_ = erlpack.pack(value)
        etf_path = os.path.join(FIXTURES_DIR, f"{name}.etf")
        txt_path = os.path.join(FIXTURES_DIR, f"{name}.txt")
        with open(etf_path, "wb") as f:
            f.write(bytes_)
        hex_dump = " ".join(f"{b:02x}" for b in bytes_)
        with open(txt_path, "w", encoding="utf-8") as f:
            f.write(f"# {name}\n")
            f.write(f"\n")
            f.write(f"Python source: erlpack.pack({value!r})\n")
            f.write(f"\n")
            f.write(f"Wire bytes ({len(bytes_)} total):\n")
            f.write(f"    {hex_dump}\n")
            f.write(f"\n")
            f.write(f"Expected Term:\n")
            f.write(f"    {description}\n")
        print(f"wrote {name}.etf ({len(bytes_)} bytes) / {name}.txt")
    return 0


if __name__ == "__main__":
    sys.exit(main())
