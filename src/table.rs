use std::ops::Deref;

#[macro_export]
macro_rules! row {
    ($($value:expr),* $(,)?) => {
        [
            $(
                $value.to_string()
            ),*
        ]
    };
}

pub use row;

#[derive(Debug)]
pub struct Table<const N: usize> {
    pub header: [String; N],
    pub abbr: [String; N],
    pub col_class: [&'static str; N],
    // (cells, first cell class)
    pub rows: Vec<Row<N>>,
}

impl<const N: usize> Table<N>
where
    [String; N]: Default,
{
    pub fn new(
        header_abbr: [(impl ToString, impl ToString); N],
        col_class: &'static str,
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
            col_class: [col_class; N],
            rows: Vec::new(),
        }
    }

    pub fn push(&mut self, data: [String; N]) {
        self.rows.push(Row {
            data,
            ..Default::default()
        });
    }

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

    pub fn insert<const M: usize, const Z: usize>(self, index: usize, other: Table<M>) -> Table<Z>
    where
        [String; Z]: Default,
        [&'static str; Z]: Default,
    {
        Table {
            header: array_insert(self.header, other.header, index),
            abbr: array_insert(self.abbr, other.abbr, index),
            col_class: array_insert(self.col_class, other.col_class, index),
            rows: self
                .rows
                .into_iter()
                .zip(other.rows)
                .map(|(a, b)| a.insert(index, b))
                .collect(),
        }
    }

    pub fn with_totals<const S: usize>(self, totals: [impl ToString; S]) -> TotalsTable<N, S>
    where
        [String; S]: Default,
    {
        let mut string_totals: [String; S] = Default::default();
        for (i, total) in totals.into_iter().enumerate() {
            string_totals[i] = total.to_string();
        }
        TotalsTable {
            table: self,
            totals: string_totals,
        }
    }
}

#[derive(Debug)]
pub struct Row<const N: usize> {
    pub data: [String; N],
    pub href: [String; N],
    pub class: &'static str,
}

impl<const N: usize> Row<N>
where
    [String; N]: Default,
{
    fn insert<const M: usize, const Z: usize>(self, index: usize, other: Row<M>) -> Row<Z>
    where
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

#[derive(Debug)]
pub struct TotalsTable<const N: usize, const S: usize> {
    pub table: Table<N>,
    pub totals: [String; S],
}

impl<const N: usize, const S: usize> Deref for TotalsTable<N, S> {
    type Target = Table<N>;

    fn deref(&self) -> &Table<N> {
        &self.table
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
