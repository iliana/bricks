use fraction::{Fraction, Zero};
use std::fmt::{self, Display};

#[derive(Debug, Clone, Copy)]
pub struct Pct<const PRECISION: u8>(pub Fraction);

impl<const PRECISION: u8> Pct<PRECISION> {
    pub fn new<T>(numerator: T, denominator: T) -> Pct<PRECISION>
    where
        T: Into<u64>,
    {
        Pct(Fraction::new(numerator, denominator))
    }
}

impl<const PRECISION: u8> Display for Pct<PRECISION> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_infinite() {
            write!(f, "inf")
        } else if self.0.is_nan() {
            write!(f, "NaN")
        } else if PRECISION == 0 {
            write!(f, "{}", self.0.round())
        } else {
            let trunc = self.0.trunc();
            if PRECISION < 3 || trunc > Fraction::zero() {
                write!(f, "{}", trunc)?;
            }
            let mult = 10_u64.pow(PRECISION.into()).into();
            let fract = (self.0.fract() * mult).round();
            write!(f, ".{:0>width$}", fract, width = PRECISION.into())
        }
    }
}

#[cfg(test)]
#[test]
fn test() {
    let obp: Pct<3> = Pct::new(256u16, 597);
    let slg: Pct<3> = Pct::new(300u16, 488);
    let ops: Pct<3> = Pct(obp.0 + slg.0);
    assert_eq!(obp.to_string(), ".429");
    assert_eq!(slg.to_string(), ".615");
    assert_eq!(ops.to_string(), "1.044");

    let n: Pct<1> = Pct::new(66u16, 99);
    assert_eq!(n.to_string(), "0.7");

    let n: Pct<0> = Pct::new(5010u16, 100);
    assert_eq!(n.to_string(), "50");
}
