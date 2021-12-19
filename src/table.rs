use crate::percentage::Pct;
use derive_more::{Display, From};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

#[macro_export]
macro_rules! row {
    ($($value:expr),* $(,)?) => {
        [
            $(
                $crate::table::Value::from($value)
            ),*
        ]
    };
}

pub use row;

#[derive(Debug)]
pub struct Table<const N: usize> {
    pub header: [String; N],
    pub abbr: [String; N],
    pub skip: Vec<usize>,
    pub col_class: [&'static str; N],
    pub sort_method: [&'static str; N],
    // (cells, first cell class)
    pub rows: Vec<Row<N>>,
}

impl<const N: usize> Table<N>
where
    [Value; N]: Default,
    [String; N]: Default,
{
    pub fn new(
        header_abbr: [(impl ToString, impl ToString); N],
        col_class: &'static str,
        sort_method: &'static str,
    ) -> Table<N> {
        let mut header: [String; N] = Default::default();
        let mut abbr: [String; N] = Default::default();
        for (i, (h, a)) in header_abbr.into_iter().enumerate() {
            header[i] = h.to_string();
            abbr[i] = a.to_string();
        }
        Table {
            header,
            abbr,
            skip: Vec::new(),
            col_class: [col_class; N],
            sort_method: [sort_method; N],
            rows: Vec::new(),
        }
    }

    pub fn push(&mut self, data: [Value; N]) {
        self.rows.push(Row {
            data,
            ..Default::default()
        });
    }

    pub fn insert<const M: usize, const Z: usize>(self, index: usize, other: Table<M>) -> Table<Z>
    where
        [Value; Z]: Default,
        [String; Z]: Default,
        [&'static str; Z]: Default,
    {
        Table {
            header: array_insert(self.header, other.header, index),
            abbr: array_insert(self.abbr, other.abbr, index),
            skip: self.skip,
            col_class: array_insert(self.col_class, other.col_class, index),
            sort_method: array_insert(self.sort_method, other.sort_method, index),
            rows: self
                .rows
                .into_iter()
                .zip(other.rows)
                .map(|(a, b)| a.insert(index, b))
                .collect(),
        }
    }
}

impl<const N: usize> Table<N> {
    pub fn set_class(&mut self, class: &'static str) {
        if let Some(row) = self.rows.last_mut() {
            row.class = class;
        }
    }

    pub fn set_href(&mut self, index: usize, href: impl ToString) {
        if let Some(row) = self.rows.last_mut() {
            row.href[index] = href.to_string();
        }
    }

    pub fn skip(&mut self, column: &str) -> &mut Table<N> {
        if let Some(index) = self
            .abbr
            .iter()
            .position(|x| x == column)
            .or_else(|| self.header.iter().position(|x| x == column))
        {
            self.skip.push(index);
        }
        self
    }

    pub fn not_skip(&self, index: &usize) -> bool {
        !self.skip.contains(index)
    }
}

#[derive(Debug)]
pub struct Row<const N: usize> {
    pub data: [Value; N],
    pub href: [String; N],
    pub class: &'static str,
}

impl<const N: usize> Row<N>
where
    [Value; N]: Default,
    [String; N]: Default,
{
    fn insert<const M: usize, const Z: usize>(self, index: usize, other: Row<M>) -> Row<Z>
    where
        [Value; Z]: Default,
        [String; Z]: Default,
    {
        Row {
            data: array_insert(self.data, other.data, index),
            href: array_insert(self.href, other.href, index),
            class: self.class,
        }
    }
}

impl<const N: usize> Default for Row<N>
where
    [Value; N]: Default,
    [String; N]: Default,
{
    fn default() -> Row<N> {
        Row {
            data: Default::default(),
            href: Default::default(),
            class: "",
        }
    }
}

#[derive(Debug, From, Display)]
pub enum Value {
    Pct0(Pct<0>),
    Pct1(Pct<1>),
    Pct2(Pct<2>),
    Pct3(Pct<3>),
    Str(String),
    U32(u32),
    Usize(usize),
}

impl Value {
    pub fn sort_value(&self) -> Cow<'_, str> {
        match self {
            Value::Pct0(Pct(frac))
            | Value::Pct1(Pct(frac))
            | Value::Pct2(Pct(frac))
            | Value::Pct3(Pct(frac)) => frac.to_js_number(),
            Value::Str(s) => s.into(),
            Value::U32(x) => x.to_string().into(),
            Value::Usize(x) => x.to_string().into(),
        }
    }
}

impl Default for Value {
    fn default() -> Value {
        Value::Str(String::default())
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::Str(s.to_string())
    }
}

#[derive(Debug)]
pub struct TotalsTable<const N: usize, const S: usize> {
    pub table: Table<N>,
    pub totals: [Value; S],
}

impl<const N: usize, const S: usize> Deref for TotalsTable<N, S> {
    type Target = Table<N>;

    fn deref(&self) -> &Table<N> {
        &self.table
    }
}

impl<const N: usize, const S: usize> DerefMut for TotalsTable<N, S> {
    fn deref_mut(&mut self) -> &mut Table<N> {
        &mut self.table
    }
}

/// Creates a new array with the elements of `a`, with the elements of `b` inserted at `index` of
/// `a`.
fn array_insert<T, const N: usize, const M: usize, const Z: usize>(
    a: [T; N],
    b: [T; M],
    index: usize,
) -> [T; Z]
where
    [T; Z]: Default,
{
    assert_eq!(N + M, Z);
    assert!(index <= a.len());

    let mut new: [T; Z] = Default::default();
    let mut a = a.into_iter();
    let mut i = 0;

    while i < index {
        new[i] = a.next().unwrap();
        i += 1;
    }
    for x in b {
        new[i] = x;
        i += 1;
    }
    for x in a {
        new[i] = x;
        i += 1;
    }

    new
}

#[cfg(test)]
#[test]
fn test_array_insert() {
    assert_eq!(array_insert([1, 2, 3], [], 2), [1, 2, 3]);

    assert_eq!(array_insert([1, 2, 3], [4], 0), [4, 1, 2, 3]);
    assert_eq!(array_insert([1, 2, 3], [4], 1), [1, 4, 2, 3]);
    assert_eq!(array_insert([1, 2, 3], [4], 2), [1, 2, 4, 3]);
    assert_eq!(array_insert([1, 2, 3], [4], 3), [1, 2, 3, 4]);

    assert_eq!(array_insert([1, 2], [3, 4, 5], 1), [1, 3, 4, 5, 2]);
}

#[cfg(test)]
#[test]
#[should_panic]
fn test_array_insert_panic() {
    assert_eq!(array_insert([1, 2, 3], [4], 2), [1, 2, 4, 3, 5]);
}
