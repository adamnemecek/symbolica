use std::{
    cmp::Ordering,
    fmt::{Display, Error, Formatter},
    ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign},
};

use rand::Rng;
use rug::{
    integer::IntegerExt64,
    ops::{Pow, RemRounding},
    Complete, Integer as ArbitraryPrecisionInteger,
};

use crate::utils;

use super::{
    finite_field::{FiniteField, FiniteFieldCore, ToFiniteField},
    rational::Rational,
    EuclideanDomain, Ring,
};

pub const SMALL_PRIMES: [i64; 100] = [
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191, 193,
    197, 199, 211, 223, 227, 229, 233, 239, 241, 251, 257, 263, 269, 271, 277, 281, 283, 293, 307,
    311, 313, 317, 331, 337, 347, 349, 353, 359, 367, 373, 379, 383, 389, 397, 401, 409, 419, 421,
    431, 433, 439, 443, 449, 457, 461, 463, 467, 479, 487, 491, 499, 503, 509, 521, 523, 541,
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct IntegerRing;

impl IntegerRing {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Integer {
    Natural(i64),
    Large(ArbitraryPrecisionInteger),
}

impl ToFiniteField<u32> for Integer {
    fn to_finite_field(&self, field: &FiniteField<u32>) -> <FiniteField<u32> as Ring>::Element {
        field.to_element(match self {
            &Self::Natural(n) => n.rem_euclid(field.get_prime() as i64) as u32,
            Self::Large(r) => r.mod_u(field.get_prime()),
        })
    }
}

impl ToFiniteField<u64> for Integer {
    fn to_finite_field(&self, field: &FiniteField<u64>) -> <FiniteField<u64> as Ring>::Element {
        field.to_element(match self {
            &Self::Natural(n) => {
                if field.get_prime() > i64::MAX as u64 {
                    (n as i128).rem_euclid(field.get_prime() as i128) as u64
                } else {
                    n.rem_euclid(field.get_prime() as i64) as u64
                }
            }
            Self::Large(r) => r.mod_u64(field.get_prime()),
        })
    }
}

impl Integer {
    pub fn new(num: i64) -> Self {
        Self::Natural(num)
    }

    pub fn from_large(n: ArbitraryPrecisionInteger) -> Self {
        if let Some(n) = n.to_i64() {
            Self::Natural(n)
        } else {
            Self::Large(n)
        }
    }

    pub fn from_finite_field_u32(
        field: FiniteField<u32>,
        element: &<FiniteField<u32> as Ring>::Element,
    ) -> Self {
        Self::Natural(field.from_element(*element) as i64)
    }

    pub fn to_rational(&self) -> Rational {
        match self {
            Self::Natural(n) => Rational::Natural(*n, 1),
            Self::Large(r) => Rational::Large(r.into()),
        }
    }

    pub fn is_zero(&self) -> bool {
        match self {
            Self::Natural(n) => *n == 0,
            Self::Large(_) => false,
        }
    }

    pub fn is_one(&self) -> bool {
        match self {
            Self::Natural(n) => *n == 1,
            Self::Large(_) => false,
        }
    }

    pub fn is_negative(&self) -> bool {
        match self {
            Self::Natural(n) => *n < 0,
            Self::Large(r) => ArbitraryPrecisionInteger::from(r.signum_ref()) == -1,
        }
    }

    pub fn zero() -> Self {
        Self::Natural(0)
    }

    pub fn one() -> Self {
        Self::Natural(1)
    }

    pub fn abs(&self) -> Self {
        match self {
            Self::Natural(n) => {
                if *n == i64::MIN {
                    Self::Large(ArbitraryPrecisionInteger::from(*n).abs())
                } else {
                    Self::Natural(n.abs())
                }
            }
            Self::Large(n) => Self::Large(n.clone().abs()),
        }
    }

    pub fn abs_cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Natural(n1), Self::Natural(n2)) => {
                if n1 == n2 {
                    Ordering::Equal
                } else if *n1 == i64::MIN {
                    Ordering::Greater
                } else {
                    n1.abs().cmp(&n2.abs())
                }
            }
            (Self::Natural(n1), Self::Large(n2)) => {
                if *n1 == i64::MIN {
                    ArbitraryPrecisionInteger::from(*n1).as_abs().cmp(n2)
                } else {
                    n2.as_abs()
                        .partial_cmp(&n1.abs())
                        .unwrap_or(Ordering::Equal)
                        .reverse()
                }
            }
            (Self::Large(n1), Self::Natural(n2)) => {
                if *n1 == i64::MIN {
                    n1.as_abs()
                        .cmp(&ArbitraryPrecisionInteger::from(*n2).as_abs())
                } else {
                    n1.as_abs()
                        .partial_cmp(&n2.abs())
                        .unwrap_or(Ordering::Equal)
                }
            }
            (Self::Large(n1), Self::Large(n2)) => n1.as_abs().cmp(&n2.as_abs()),
        }
    }

    /// Compute the binomial coefficient `(n k) = n!/(k!(n-k)!)`.
    ///
    /// The implementation does not to overflow.
    pub fn binom(n: i64, mut k: i64) -> Self {
        if n < 0 || k < 0 || k > n {
            return Self::zero();
        }
        if k > n / 2 {
            k = n - k
        }
        let mut res = Self::one();
        for i in 1..=k {
            res *= n - k + i;
            res /= i;
        }
        res
    }

    /// Compute the multinomial coefficient `(k_1+...+k_n)!/(k_1!*...*k_n!)`
    ///
    /// The implementation does not to overflow.
    pub fn multinom(k: &[u32]) -> Self {
        let mut mcr = Self::one();
        let mut accum = 0i64;
        for v in k {
            let Some(res) = accum.checked_add(*v as i64) else {
                panic!("Sum of occurrences exceeds i64: {:?}", k)
            };
            accum = res;
            mcr *= &Self::binom(accum, *v as i64);
        }
        mcr
    }

    pub fn pow(&self, e: u64) -> Self {
        assert!(
            e <= u32::MAX as u64,
            "Power of exponentation is larger than 2^32: {}",
            e
        );
        let e = e as u32;

        if e == 0 {
            return Self::one();
        }

        match self {
            Self::Natural(n1) => {
                if let Some(pn) = n1.checked_pow(e) {
                    Self::Natural(pn)
                } else {
                    Self::Large(ArbitraryPrecisionInteger::from(*n1).pow(e))
                }
            }
            Self::Large(r) => Self::Large(r.pow(e).into()),
        }
    }

    /// Use Garner's algorithm for the Chinese remainder theorem
    /// to reconstruct an x that satisfies n1 = x % p1 and n2 = x % p2.
    /// The x will be in the range [-p1*p2/2,p1*p2/2].
    pub fn chinese_remainder(n1: Self, n2: Self, p1: Self, p2: Self) -> Self {
        // make sure n1 < n2
        if match (&n1, &n2) {
            (Self::Natural(n1), Self::Natural(n2)) => n1 > n2,
            (Self::Natural(_), Self::Large(_)) => false,
            (Self::Large(_), Self::Natural(_)) => true,
            (Self::Large(r1), Self::Large(r2)) => r1 > r2,
        } {
            return Self::chinese_remainder(n2, n1, p2, p1);
        }

        let p1 = match p1 {
            Self::Natural(n) => n.into(),
            Self::Large(r) => r,
        };
        let p2 = match p2 {
            Self::Natural(n) => n.into(),
            Self::Large(r) => r,
        };

        let n1 = match n1 {
            Self::Natural(n) => n.into(),
            Self::Large(r) => r,
        };
        let n2 = match n2 {
            Self::Natural(n) => n.into(),
            Self::Large(r) => r,
        };

        // convert to mixed-radix notation
        let gamma1 = (p1.clone() % p2.clone())
            .invert(&p2)
            .unwrap_or_else(|_| panic!("Could not invert {} in {}", p1, p2));

        let v1 = ((n2 - n1.clone()) * gamma1) % p2.clone();

        // convert to standard representation
        let r = v1 * p1.clone() + n1;

        let res = if r.clone() * 2 > p1.clone() * p2.clone() {
            r - p1 * p2
        } else {
            r
        };

        Self::from_large(res)
    }
}

impl Display for Integer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Natural(n) => n.fmt(f),
            Self::Large(r) => r.fmt(f),
        }
    }
}

impl Display for IntegerRing {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

impl PartialOrd for Integer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Self::Natural(n1), Self::Natural(n2)) => n1.partial_cmp(n2),
            (Self::Natural(n1), Self::Large(n2)) => n1.partial_cmp(n2),
            (Self::Large(n1), Self::Natural(n2)) => n1.partial_cmp(n2),
            (Self::Large(n1), Self::Large(n2)) => n1.partial_cmp(n2),
        }
    }
}

impl Ord for Integer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Ring for IntegerRing {
    type Element = Integer;

    #[inline]
    fn add(&self, a: &Self::Element, b: &Self::Element) -> Self::Element {
        a + b
    }

    #[inline]
    fn sub(&self, a: &Self::Element, b: &Self::Element) -> Self::Element {
        a - b
    }

    #[inline]
    fn mul(&self, a: &Self::Element, b: &Self::Element) -> Self::Element {
        a * b
    }

    #[inline]
    fn add_assign(&self, a: &mut Self::Element, b: &Self::Element) {
        *a += b;
    }

    #[inline]
    fn sub_assign(&self, a: &mut Self::Element, b: &Self::Element) {
        *a -= b;
    }

    #[inline]
    fn mul_assign(&self, a: &mut Self::Element, b: &Self::Element) {
        *a *= b;
    }

    #[inline(always)]
    fn add_mul_assign(&self, a: &mut Self::Element, b: &Self::Element, c: &Self::Element) {
        match a {
            Integer::Natural(n) => {
                let l = match (b, c) {
                    (Integer::Natural(b), Integer::Natural(c)) => {
                        if let Some(d) = b.checked_mul(*c) {
                            if let Some(n2) = n.checked_add(d) {
                                *n = n2;
                                return;
                            } else {
                                *n + ArbitraryPrecisionInteger::from(d)
                            }
                        } else {
                            *n + ArbitraryPrecisionInteger::from(*b) * c
                        }
                    }
                    (Integer::Natural(b), Integer::Large(c)) => *n + (b * c).complete(),
                    (Integer::Large(b), Integer::Natural(c)) => *n + (b * c).complete(),
                    (Integer::Large(b), Integer::Large(c)) => *n + (b * c).complete(),
                };

                *a = if let Some(n) = l.to_i64() {
                    Integer::Natural(n)
                } else {
                    Integer::Large(l)
                };
            }
            Integer::Large(l) => {
                match (b, c) {
                    (Integer::Natural(b), Integer::Natural(c)) => {
                        if let Some(n) = b.checked_mul(*c) {
                            l.add_assign(n);
                        } else {
                            l.add_assign(ArbitraryPrecisionInteger::from(*b) * c);
                        }
                    }
                    (Integer::Natural(b), Integer::Large(c)) => {
                        l.add_assign(*b * c);
                    }
                    (Integer::Large(b), Integer::Natural(c)) => {
                        l.add_assign(b * *c);
                    }
                    (Integer::Large(b), Integer::Large(c)) => {
                        l.add_assign(b * c);
                    }
                }

                if let Some(n) = l.to_i64() {
                    *a = Integer::Natural(n);
                }
            }
        }
    }

    #[inline(always)]
    fn sub_mul_assign(&self, a: &mut Self::Element, b: &Self::Element, c: &Self::Element) {
        match a {
            Integer::Natural(n) => {
                let l = match (b, c) {
                    (Integer::Natural(b), Integer::Natural(c)) => {
                        if let Some(d) = b.checked_mul(*c) {
                            if let Some(n2) = n.checked_sub(d) {
                                *n = n2;
                                return;
                            } else {
                                *n - ArbitraryPrecisionInteger::from(d)
                            }
                        } else {
                            *n - ArbitraryPrecisionInteger::from(*b) * c
                        }
                    }
                    (Integer::Natural(b), Integer::Large(c)) => *n - (b * c).complete(),
                    (Integer::Large(b), Integer::Natural(c)) => *n - (b * c).complete(),
                    (Integer::Large(b), Integer::Large(c)) => *n - (b * c).complete(),
                };

                *a = if let Some(n) = l.to_i64() {
                    Integer::Natural(n)
                } else {
                    Integer::Large(l)
                };
            }
            Integer::Large(l) => {
                match (b, c) {
                    (Integer::Natural(b), Integer::Natural(c)) => {
                        if let Some(n) = b.checked_mul(*c) {
                            l.sub_assign(n);
                        } else {
                            l.sub_assign(ArbitraryPrecisionInteger::from(*b) * c);
                        }
                    }
                    (Integer::Natural(b), Integer::Large(c)) => {
                        l.sub_assign(*b * c);
                    }
                    (Integer::Large(b), Integer::Natural(c)) => {
                        l.sub_assign(b * *c);
                    }
                    (Integer::Large(b), Integer::Large(c)) => {
                        l.sub_assign(b * c);
                    }
                }

                if let Some(n) = l.to_i64() {
                    *a = Integer::Natural(n);
                }
            }
        }
    }

    #[inline]
    fn neg(&self, a: &Self::Element) -> Self::Element {
        -a
    }

    #[inline]
    fn zero(&self) -> Self::Element {
        Integer::zero()
    }

    #[inline]
    fn one(&self) -> Self::Element {
        Integer::one()
    }

    #[inline]
    fn pow(&self, b: &Self::Element, e: u64) -> Self::Element {
        b.pow(e)
    }

    #[inline]
    fn is_zero(a: &Self::Element) -> bool {
        match a {
            Integer::Natural(r) => *r == 0,
            Integer::Large(_) => false,
        }
    }

    #[inline]
    fn is_one(&self, a: &Self::Element) -> bool {
        match a {
            Integer::Natural(r) => *r == 1,
            Integer::Large(_) => false,
        }
    }

    fn get_unit(&self, a: &Self::Element) -> Self::Element {
        match a.cmp(&Integer::zero()) {
            Ordering::Less => Integer::Natural(-1),
            Ordering::Equal => Integer::zero(),
            Ordering::Greater => Integer::one(),
        }
    }

    fn get_inv_unit(&self, a: &Self::Element) -> Self::Element {
        self.get_unit(a)
    }

    fn sample(&self, rng: &mut impl rand::RngCore, range: (i64, i64)) -> Self::Element {
        let r = rng.gen_range(range.0..range.1);
        Integer::Natural(r)
    }

    fn fmt_display(&self, element: &Self::Element, f: &mut Formatter<'_>) -> Result<(), Error> {
        element.fmt(f)
    }
}

impl EuclideanDomain for IntegerRing {
    fn rem(&self, a: &Self::Element, b: &Self::Element) -> Self::Element {
        match (a, b) {
            (Integer::Natural(a), Integer::Natural(b)) => {
                if let Some(r) = a.checked_rem_euclid(*b) {
                    Integer::Natural(r)
                } else {
                    Integer::from_large(
                        ArbitraryPrecisionInteger::from(*a)
                            .rem_euc(ArbitraryPrecisionInteger::from(*b)),
                    )
                }
            }
            (Integer::Natural(a), Integer::Large(b)) => {
                Integer::from_large(ArbitraryPrecisionInteger::from(*a).rem_euc(b))
            }
            (Integer::Large(a), Integer::Natural(b)) => {
                Integer::from_large(a.rem_euc(ArbitraryPrecisionInteger::from(*b)))
            }
            (Integer::Large(a), Integer::Large(b)) => Integer::from_large(a.rem_euc(b).into()),
        }
    }

    fn quot_rem(&self, a: &Self::Element, b: &Self::Element) -> (Self::Element, Self::Element) {
        match (a, b) {
            (Integer::Natural(aa), Integer::Natural(bb)) => {
                if let Some(r) = aa.checked_div_euclid(*bb) {
                    (Integer::Natural(r), a - &(b * &Integer::Natural(r)))
                } else {
                    let r = ArbitraryPrecisionInteger::from(*aa)
                        .div_rem_euc(ArbitraryPrecisionInteger::from(*bb));
                    (Integer::from_large(r.0), Integer::from_large(r.1))
                }
            }
            (Integer::Natural(a), Integer::Large(b)) => {
                let r = ArbitraryPrecisionInteger::from(*a).div_rem_euc(b.clone());
                (Integer::from_large(r.0), Integer::from_large(r.1))
            }
            (Integer::Large(a), Integer::Natural(b)) => {
                let r = a.clone().div_rem_euc(ArbitraryPrecisionInteger::from(*b));
                (Integer::from_large(r.0), Integer::from_large(r.1))
            }
            (Integer::Large(a), Integer::Large(b)) => {
                let r = a.clone().div_rem_euc(b.clone());
                (Integer::from_large(r.0), Integer::from_large(r.1))
            }
        }
    }

    fn gcd(&self, a: &Self::Element, b: &Self::Element) -> Self::Element {
        match (a, b) {
            (Integer::Natural(n1), Integer::Natural(n2)) => {
                Integer::Natural(utils::gcd_signed(*n1, *n2))
            }
            (Integer::Natural(n1), Integer::Large(r2))
            | (Integer::Large(r2), Integer::Natural(n1)) => {
                let r1 = ArbitraryPrecisionInteger::from(*n1);
                Integer::from_large(r1.gcd(r2))
            }
            (Integer::Large(r1), Integer::Large(r2)) => Integer::from_large(r1.clone().gcd(r2)),
        }
    }
}

impl<'a, 'b> Add<&'b Integer> for &'a Integer {
    type Output = Integer;

    fn add(self, rhs: &'b Integer) -> Integer {
        match (self, rhs) {
            (Integer::Natural(n1), Integer::Natural(n2)) => {
                if let Some(num) = n1.checked_add(*n2) {
                    Integer::Natural(num)
                } else {
                    Integer::Large(ArbitraryPrecisionInteger::from(*n1) + *n2)
                }
            }
            (Integer::Natural(n1), Integer::Large(r2))
            | (Integer::Large(r2), Integer::Natural(n1)) => Integer::from_large((*n1 + r2).into()),
            (Integer::Large(r1), Integer::Large(r2)) => Integer::from_large((r1 + r2).into()),
        }
    }
}

impl<'a, 'b> Sub<&'b Integer> for &'a Integer {
    type Output = Integer;

    fn sub(self, rhs: &'b Integer) -> Integer {
        match (self, rhs) {
            (Integer::Natural(n1), Integer::Natural(n2)) => {
                if let Some(num) = n1.checked_sub(*n2) {
                    Integer::Natural(num)
                } else {
                    Integer::Large(ArbitraryPrecisionInteger::from(*n1) - *n2)
                }
            }
            (Integer::Natural(n1), Integer::Large(r2)) => Integer::from_large((*n1 - r2).into()),
            (Integer::Large(r1), Integer::Natural(n2)) => Integer::from_large((r1 - *n2).into()),
            (Integer::Large(r1), Integer::Large(r2)) => Integer::from_large((r1 - r2).into()),
        }
    }
}

impl<'a, 'b> Mul<&'b Integer> for &'a Integer {
    type Output = Integer;

    fn mul(self, rhs: &'b Integer) -> Integer {
        match (self, rhs) {
            (Integer::Natural(n1), Integer::Natural(n2)) => {
                if let Some(nn) = n1.checked_mul(*n2) {
                    Integer::Natural(nn)
                } else {
                    Integer::Large(ArbitraryPrecisionInteger::from(*n1) * *n2)
                }
            }
            (Integer::Natural(n1), Integer::Large(r2))
            | (Integer::Large(r2), Integer::Natural(n1)) => Integer::from_large((n1 * r2).into()),
            (Integer::Large(r1), Integer::Large(r2)) => Integer::from_large((r1 * r2).into()),
        }
    }
}

impl<'a, 'b> Div<&'b Integer> for &'a Integer {
    type Output = Integer;

    fn div(self, rhs: &'b Integer) -> Integer {
        match (self, rhs) {
            (Integer::Natural(n1), Integer::Natural(n2)) => {
                if let Some(nn) = n1.checked_div(*n2) {
                    Integer::Natural(nn)
                } else {
                    Integer::Large(ArbitraryPrecisionInteger::from(*n1) / *n2)
                }
            }
            (Integer::Natural(n1), Integer::Large(r2)) => Integer::from_large((*n1 / r2).into()),
            (Integer::Large(r1), Integer::Natural(n2)) => Integer::from_large((r1 / *n2).into()),
            (Integer::Large(r1), Integer::Large(r2)) => Integer::from_large((r1 / r2).into()),
        }
    }
}

impl<'a> Add<i64> for &'a Integer {
    type Output = Integer;

    fn add(self, rhs: i64) -> Integer {
        match self {
            Integer::Natural(n1) => {
                if let Some(num) = n1.checked_add(rhs) {
                    Integer::Natural(num)
                } else {
                    Integer::Large(ArbitraryPrecisionInteger::from(*n1) + rhs)
                }
            }
            Integer::Large(n1) => Integer::from_large((n1 + rhs).into()),
        }
    }
}

impl<'a> Sub<i64> for &'a Integer {
    type Output = Integer;

    fn sub(self, rhs: i64) -> Integer {
        match self {
            Integer::Natural(n1) => {
                if let Some(num) = n1.checked_sub(rhs) {
                    Integer::Natural(num)
                } else {
                    Integer::Large(ArbitraryPrecisionInteger::from(*n1) - rhs)
                }
            }
            Integer::Large(n1) => Integer::from_large((n1 - rhs).into()),
        }
    }
}

impl<'a> Mul<i64> for &'a Integer {
    type Output = Integer;

    fn mul(self, rhs: i64) -> Integer {
        match self {
            Integer::Natural(n1) => {
                if let Some(num) = n1.checked_mul(rhs) {
                    Integer::Natural(num)
                } else {
                    Integer::Large(ArbitraryPrecisionInteger::from(*n1) * rhs)
                }
            }
            Integer::Large(n1) => Integer::from_large((n1 * rhs).into()),
        }
    }
}

impl<'a> Div<i64> for &'a Integer {
    type Output = Integer;

    fn div(self, rhs: i64) -> Integer {
        match self {
            Integer::Natural(n1) => {
                if let Some(num) = n1.checked_div(rhs) {
                    Integer::Natural(num)
                } else {
                    Integer::Large(ArbitraryPrecisionInteger::from(*n1) / rhs)
                }
            }
            Integer::Large(n1) => Integer::from_large((n1 / rhs).into()),
        }
    }
}

impl<'a> AddAssign<&'a Self> for Integer {
    fn add_assign(&mut self, rhs: &'a Self) {
        match self {
            Self::Natural(n1) => match rhs {
                Self::Natural(n2) => {
                    if let Some(nn) = n1.checked_add(*n2) {
                        *n1 = nn;
                    } else {
                        let mut r1 = ArbitraryPrecisionInteger::from(*n1);
                        r1.add_assign(*n2);
                        *self = Self::Large(r1)
                    }
                }
                Self::Large(r2) => {
                    let mut r1 = ArbitraryPrecisionInteger::from(*n1);
                    r1.add_assign(r2);
                    *self = Self::from_large(r1)
                }
            },
            Self::Large(r1) => match rhs {
                Self::Natural(n2) => {
                    r1.add_assign(*n2);
                    if let Some(n) = r1.to_i64() {
                        *self = Self::Natural(n);
                    }
                }
                Self::Large(r2) => {
                    r1.add_assign(r2);
                    if let Some(n) = r1.to_i64() {
                        *self = Self::Natural(n);
                    }
                }
            },
        };
    }
}

impl<'a> SubAssign<&'a Self> for Integer {
    fn sub_assign(&mut self, rhs: &'a Self) {
        match self {
            Self::Natural(n1) => match rhs {
                Self::Natural(n2) => {
                    if let Some(nn) = n1.checked_sub(*n2) {
                        *n1 = nn;
                    } else {
                        let mut r1 = ArbitraryPrecisionInteger::from(*n1);
                        r1.sub_assign(*n2);
                        *self = Self::Large(r1)
                    }
                }
                Self::Large(r2) => {
                    let mut r1 = ArbitraryPrecisionInteger::from(*n1);
                    r1.sub_assign(r2);
                    *self = Self::from_large(r1)
                }
            },
            Self::Large(r1) => match rhs {
                Self::Natural(n2) => {
                    r1.sub_assign(*n2);
                    if let Some(n) = r1.to_i64() {
                        *self = Self::Natural(n);
                    }
                }
                Self::Large(r2) => {
                    r1.sub_assign(r2);
                    if let Some(n) = r1.to_i64() {
                        *self = Self::Natural(n);
                    }
                }
            },
        };
    }
}

impl<'a> MulAssign<&'a Self> for Integer {
    fn mul_assign(&mut self, rhs: &'a Self) {
        match self {
            Self::Natural(n1) => match rhs {
                Self::Natural(n2) => {
                    if let Some(nn) = n1.checked_mul(*n2) {
                        *n1 = nn;
                    } else {
                        let mut r1 = ArbitraryPrecisionInteger::from(*n1);
                        r1.mul_assign(*n2);
                        *self = Self::from_large(r1)
                    }
                }
                Self::Large(r2) => {
                    let mut r1 = ArbitraryPrecisionInteger::from(*n1);
                    r1.mul_assign(r2);
                    *self = Self::from_large(r1)
                }
            },
            Self::Large(r1) => match rhs {
                Self::Natural(n2) => {
                    r1.mul_assign(*n2);
                    if let Some(n) = r1.to_i64() {
                        *self = Self::Natural(n);
                    }
                }
                Self::Large(r2) => {
                    r1.mul_assign(r2);
                    if let Some(n) = r1.to_i64() {
                        *self = Self::Natural(n);
                    }
                }
            },
        };
    }
}

impl<'a> DivAssign<&'a Self> for Integer {
    fn div_assign(&mut self, rhs: &'a Self) {
        match self {
            Self::Natural(n1) => match rhs {
                Self::Natural(n2) => {
                    if let Some(nn) = n1.checked_div(*n2) {
                        *n1 = nn;
                    } else {
                        let mut r1 = ArbitraryPrecisionInteger::from(*n1);
                        r1.div_assign(*n2);
                        *self = Self::Large(r1)
                    }
                }
                Self::Large(r2) => {
                    let mut r1 = ArbitraryPrecisionInteger::from(*n1);
                    r1.div_assign(r2);
                    *self = Self::Large(r1)
                }
            },
            Self::Large(r1) => match rhs {
                Self::Natural(n2) => {
                    r1.div_assign(*n2);
                    if let Some(n) = r1.to_i64() {
                        *self = Self::Natural(n);
                    }
                }
                Self::Large(r2) => {
                    r1.div_assign(r2);
                    if let Some(n) = r1.to_i64() {
                        *self = Self::Natural(n);
                    }
                }
            },
        };
    }
}

impl MulAssign<i64> for Integer {
    #[inline]
    fn mul_assign(&mut self, rhs: i64) {
        *self = (&*self) * rhs;
    }
}

impl DivAssign<i64> for Integer {
    #[inline]
    fn div_assign(&mut self, rhs: i64) {
        *self = (&*self) / rhs;
    }
}

impl<'a> Neg for &'a Integer {
    type Output = Integer;

    fn neg(self) -> Self::Output {
        match self {
            Integer::Natural(n) => {
                if let Some(neg) = n.checked_neg() {
                    Integer::Natural(neg)
                } else {
                    Integer::Large(ArbitraryPrecisionInteger::from(*n).neg())
                }
            }
            Integer::Large(r) => Integer::from_large(r.neg().into()),
        }
    }
}
