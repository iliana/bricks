use crate::fraction::Fraction;
use std::fmt::{self, Display};

#[derive(Debug, Clone, Copy)]
pub struct Pct<const PRECISION: u8>(pub Fraction);

impl<const PRECISION: u8> Pct<PRECISION> {
    pub fn new<N, D>(numerator: N, denominator: D) -> Pct<PRECISION>
    where
        i64: From<N>,
        u64: From<D>,
    {
        Pct(Fraction::new(i64::from(numerator), u64::from(denominator)))
    }
}

impl<const PRECISION: u8> Display for Pct<PRECISION> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if PRECISION < 3 {
            write!(f, "{:0>#.width$}", self.0, width = PRECISION.into())
        } else {
            write!(f, "{:0>.width$}", self.0, width = PRECISION.into())
        }
    }
}

#[cfg(test)]
#[test]
fn test() {
    let obp: Pct<3> = Pct::new(256, 597u16);
    let slg: Pct<3> = Pct::new(300, 488u16);
    let ops: Pct<3> = Pct(obp.0 + slg.0);
    assert_eq!(obp.to_string(), ".429");
    assert_eq!(slg.to_string(), ".615");
    assert_eq!(ops.to_string(), "1.044");

    let n: Pct<1> = Pct::new(66, 99u16);
    assert_eq!(n.to_string(), "0.7");

    let n: Pct<0> = Pct::new(5001, 100u16);
    assert_eq!(n.to_string(), "50");
    let n: Pct<0> = Pct::new(5000, 100u16);
    assert_eq!(n.to_string(), "50");
    let n: Pct<0> = Pct::new(4999, 100u16);
    assert_eq!(n.to_string(), "50");
    let n: Pct<0> = Pct::new(4950, 100u16);
    assert_eq!(n.to_string(), "50");
    let n: Pct<0> = Pct::new(4949, 100u16);
    assert_eq!(n.to_string(), "49");

    let n: Pct<2> = Pct::new(19999, 10000u16);
    assert_eq!(n.to_string(), "2.00");
    let n: Pct<2> = Pct::new(20001, 10000u16);
    assert_eq!(n.to_string(), "2.00");
}
