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

    pub fn set_first_cell_class(&mut self, class: &'static str) {
        if let Some(row) = self.rows.last_mut() {
            row.first_cell_class = class;
        }
    }

    pub fn set_href(&mut self, index: usize, href: impl ToString) {
        if let Some(row) = self.rows.last_mut() {
            row.href[index] = href.to_string();
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
    pub first_cell_class: &'static str,
}

impl<const N: usize> Default for Row<N>
where
    [String; N]: Default,
{
    fn default() -> Row<N> {
        Row {
            data: Default::default(),
            href: Default::default(),
            first_cell_class: "",
        }
    }
}

#[derive(Debug)]
pub struct TotalsTable<const N: usize, const S: usize> {
    table: Table<N>,
    pub totals: [String; S],
}

impl<const N: usize, const S: usize> Deref for TotalsTable<N, S> {
    type Target = Table<N>;

    fn deref(&self) -> &Table<N> {
        &self.table
    }
}
