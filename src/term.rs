#[derive(Clone, Debug, PartialEq)]
pub enum Term {
    Atom(String),
    Integer(i32),
    Float(f64),
    Big {
        sign: bool,
        digits: Vec<u8>,
    },
    Binary(Vec<u8>),
    Tuple(Vec<Term>),
    List(Vec<Term>),
    Map(Vec<(Term, Term)>),
}
