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
    pub rows: Vec<([String; N], &'static str)>,
}
