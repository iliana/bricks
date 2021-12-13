/// lol
use gcd::Gcd;
use std::cmp::Ordering;
use std::fmt::{self, Display};
use std::ops::{Add, Div, Mul, Sub};

#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Fraction {
    pub numer: i64,
    pub denom: u64,
}

impl Fraction {
    pub fn new(numer: i64, denom: u64) -> Fraction {
        if denom == 0 {
            if numer == 0 {
                // NaN
                Fraction { numer: 0, denom: 0 }
            } else {
                // infinity
                Fraction {
                    numer: numer.signum(),
                    denom: 0,
                }
            }
        } else {
            let gcd = numer.unsigned_abs().gcd(denom);
            Fraction {
                numer: numer / (gcd as i64),
                denom: denom / gcd,
            }
        }
    }

    pub fn to_f64(self) -> f64 {
        self.numer as f64 / self.denom as f64
    }

    fn is_overflow(self) -> bool {
        self.numer == i64::MAX && self.denom == u64::MAX
    }

    pub fn round(self) -> i64 {
        assert!(self.denom != 0);

        let numer = self.numer.unsigned_abs();
        let quo = numer / self.denom;
        let rem = numer % self.denom;

        let result = match (rem.cmp(&(self.denom >> 1)), self.denom & 1 == 0) {
            (Ordering::Greater, _) | (Ordering::Equal, true) => quo.checked_add(1).unwrap(),
            _ => quo,
        } as i64;

        (result as i64) * self.numer.signum()
    }
}

impl From<i64> for Fraction {
    fn from(numer: i64) -> Fraction {
        Fraction { numer, denom: 1 }
    }
}

fn mul(x: i64, y: u64) -> Option<i64> {
    i64::try_from(x.unsigned_abs().checked_mul(y)?)
        .ok()?
        .checked_mul(x.signum())
}

macro_rules! overflow {
    ($expr:expr) => {
        match $expr {
            Some(x) => x,
            None => {
                return Fraction {
                    numer: i64::MAX,
                    denom: u64::MAX,
                }
            }
        }
    };
}

impl Add for Fraction {
    type Output = Fraction;

    fn add(self, other: Fraction) -> Fraction {
        if self.denom == 0 && other.denom == 0 && self.numer.signum() == other.numer.signum() {
            self
        } else {
            // a/b + c/d = (ad + bc)/cd
            let ad = overflow!(mul(self.numer, other.denom));
            let bc = overflow!(mul(other.numer, self.denom));
            let cd = overflow!(self.denom.checked_mul(other.denom));

            Fraction::new(overflow!(ad.checked_add(bc)), cd)
        }
    }
}

impl Sub for Fraction {
    type Output = Fraction;

    fn sub(self, other: Fraction) -> Fraction {
        self + Fraction {
            numer: -other.numer,
            denom: other.denom,
        }
    }
}

impl Mul for Fraction {
    type Output = Fraction;

    fn mul(self, other: Fraction) -> Fraction {
        Fraction::new(
            overflow!(self.numer.checked_mul(other.numer)),
            overflow!(self.denom.checked_mul(other.denom)),
        )
    }
}

impl Div for Fraction {
    type Output = Fraction;

    fn div(self, other: Fraction) -> Fraction {
        Fraction::new(
            overflow!(overflow!(mul(self.numer, other.denom)).checked_mul(other.numer.signum())),
            overflow!(self.denom.checked_mul(other.numer.unsigned_abs())),
        )
    }
}

impl Display for Fraction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_overflow() {
            write!(f, "ovf")
        } else if self.denom == 0 {
            f.write_str(match self.numer.cmp(&0) {
                Ordering::Equal => "NaN",
                Ordering::Less => "-inf",
                Ordering::Greater => "inf",
            })
        } else {
            let leading_zero = f.alternate();
            let precision = f.precision().unwrap_or(3);

            let mult = 10_i64.pow(u32::try_from(precision).map_err(|_| fmt::Error)?);
            let mult_frac = Fraction {
                numer: mult,
                denom: 1,
            };
            let x = (*self * mult_frac).round();
            let trunc = x / mult;
            if leading_zero || trunc != 0 {
                write!(f, "{}", trunc)?;
            }
            if precision > 0 {
                write!(
                    f,
                    ".{:0>width$}",
                    (x % mult).unsigned_abs(),
                    width = precision
                )?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Fraction;
    use proptest::prelude::*;

    macro_rules! f {
        ($n:expr, $d:expr) => {{
            let numer: i64 = $n;
            let denom: u64 = $d;
            Fraction::new(numer, denom)
        }};
    }

    macro_rules! eq {
        ($oper:tt, $a:expr, $b:expr, $c:expr, $d:expr) => {{
            let frac = f!($a.into(), $b.into()) $oper f!($c.into(), $d.into());
            prop_assume!(!frac.is_overflow());
            let frac = frac.to_f64();
            let float = ($a as f64 / $b as f64) $oper ($c as f64 / $d as f64);
            if !((frac.is_nan() && float.is_nan())
                || (frac.is_infinite() && float.is_infinite() && frac.signum() == float.signum()))
            {
                float_cmp::assert_approx_eq!(f64, frac, float, epsilon = 0.00000003, ulps = 2);
            }
        }};
    }

    proptest! {
        #[test]
        fn add(a: i32, b: u32, c: i32, d: u32) {
            eq!(+, a, b, c, d);
        }

        #[test]
        fn sub(a: i32, b: u32, c: i32, d: u32) {
            eq!(-, a, b, c, d);
        }

        #[test]
        fn mul(a: i32, b: u32, c: i32, d: u32) {
            eq!(*, a, b, c, d);
        }

        #[test]
        fn div(a: i32, b: u32, c: i32, d: u32) {
            eq!(/, a, b, c, d);
        }
    }

    #[test]
    fn reduce() {
        assert_eq!(f!(4, 2), f!(2, 1));
        assert_eq!(f!(2500, 15), f!(500, 3));
        assert_eq!(f!(-777, 21), f!(-111, 3));
    }

    #[test]
    fn round() {
        assert_eq!(f!(249, 100).round(), 2);
        assert_eq!(f!(250, 100).round(), 3);
        assert_eq!(f!(251, 100).round(), 3);
        assert_eq!(f!(-249, 100).round(), -2);
        assert_eq!(f!(-250, 100).round(), -3);
        assert_eq!(f!(-251, 100).round(), -3);

        assert_eq!(f!(252, 101).round(), 2);
        assert_eq!(f!(253, 101).round(), 3);
        assert_eq!(f!(-252, 101).round(), -2);
        assert_eq!(f!(-253, 101).round(), -3);
    }

    #[test]
    fn the_bad_ones() {
        const NAN: Fraction = Fraction { numer: 0, denom: 0 };
        const INFINITY: Fraction = Fraction { numer: 1, denom: 0 };
        const NEG_INFINITY: Fraction = Fraction {
            numer: -1,
            denom: 0,
        };

        macro_rules! bad {
            (@ $x:ident $oper:tt $y:ident) => {{
                let x = f64::$x $oper f64::$y;
                let z = if x.is_nan() {
                    NAN
                } else if x.is_sign_positive() {
                    INFINITY
                } else {
                    NEG_INFINITY
                };
                assert_eq!($x $oper $y, z);
            }};

            ($x:ident, $y:ident) => {{
                bad!(@ $x + $y);
                bad!(@ $x - $y);
                bad!(@ $x * $y);
                bad!(@ $x / $y);
            }};
        }

        bad!(NAN, NAN);
        bad!(NAN, INFINITY);
        bad!(NAN, NEG_INFINITY);
        bad!(INFINITY, NAN);
        bad!(INFINITY, INFINITY);
        bad!(INFINITY, NEG_INFINITY);
        bad!(NEG_INFINITY, NAN);
        bad!(NEG_INFINITY, INFINITY);
        bad!(NEG_INFINITY, NEG_INFINITY);

        assert!(NAN.to_f64().is_nan());
        assert!(INFINITY.to_f64().is_infinite());
        assert!(INFINITY.to_f64().is_sign_positive());
        assert!(NEG_INFINITY.to_f64().is_infinite());
        assert!(NEG_INFINITY.to_f64().is_sign_negative());
    }
}
