use std::fmt::{self, Display};
use std::ops::Add;

#[derive(Debug, Clone, Copy)]
pub struct Pct<const PRECISION: u8>(pub f64);

impl<const PRECISION: u8> Pct<PRECISION> {
    pub fn new<T>(numerator: T, denominator: T) -> Pct<PRECISION>
    where
        f64: From<T>,
    {
        Pct(f64::from(numerator) / f64::from(denominator))
    }
}

impl<const PRECISION: u8> Add for Pct<PRECISION> {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Pct(self.0 + other.0)
    }
}

impl<const PRECISION: u8> Display for Pct<PRECISION> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_infinite() {
            write!(f, "inf")
        } else if self.0.is_nan() {
            write!(f, "NaN")
        } else {
            let mult_f = 10.0_f64.powi(PRECISION.into());
            let mult_i = 10_u64.pow(PRECISION.into());
            let frac = (self.0 * mult_f).round() as u64;
            if PRECISION < 3 || frac >= mult_i {
                write!(f, "{}", frac / mult_i)?;
            }
            write!(f, ".{:0>width$}", frac % mult_i, width = PRECISION.into())
        }
    }
}

#[cfg(test)]
#[test]
fn test() {
    let obp: Pct<3> = Pct::new(256, 597);
    let slg: Pct<3> = Pct::new(300, 488);
    let ops = obp + slg;
    assert_eq!(obp.to_string(), ".429");
    assert_eq!(slg.to_string(), ".615");
    assert_eq!(ops.to_string(), "1.044");
}
