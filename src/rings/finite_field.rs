use rand::Rng;
use std::fmt::{Display, Error, Formatter};
use std::hash::Hash;

use super::{EuclideanDomain, Field, Ring};

const HENSEL_LIFTING_MASK: [u8; 128] = [
    255, 85, 51, 73, 199, 93, 59, 17, 15, 229, 195, 89, 215, 237, 203, 33, 31, 117, 83, 105, 231,
    125, 91, 49, 47, 5, 227, 121, 247, 13, 235, 65, 63, 149, 115, 137, 7, 157, 123, 81, 79, 37, 3,
    153, 23, 45, 11, 97, 95, 181, 147, 169, 39, 189, 155, 113, 111, 69, 35, 185, 55, 77, 43, 129,
    127, 213, 179, 201, 71, 221, 187, 145, 143, 101, 67, 217, 87, 109, 75, 161, 159, 245, 211, 233,
    103, 253, 219, 177, 175, 133, 99, 249, 119, 141, 107, 193, 191, 21, 243, 9, 135, 29, 251, 209,
    207, 165, 131, 25, 151, 173, 139, 225, 223, 53, 19, 41, 167, 61, 27, 241, 239, 197, 163, 57,
    183, 205, 171, 1,
];

pub trait ToFiniteField<UField: FiniteFieldWorkspace>
where
    FiniteField<UField>: FiniteFieldCore<UField>,
{
    fn to_finite_field(
        &self,
        field: &FiniteField<UField>,
    ) -> <FiniteField<UField> as Ring>::Element;
}

/// A number in a finite field.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct FiniteFieldElement<UField>(pub(crate) UField);

pub trait FiniteFieldWorkspace: Clone + Copy + Display + Eq + Hash {
    /// Convert to u64.
    fn to_u64(&self) -> u64;
}

pub trait FiniteFieldCore<UField: FiniteFieldWorkspace>: Field {
    fn new(p: UField) -> Self;
    fn get_prime(&self) -> UField;
    /// Convert a number in a prime field a % n to Montgomory form.
    fn to_element(&self, a: UField) -> Self::Element;
    /// Convert a number from Montgomory form to standard form.
    fn from_element(&self, a: Self::Element) -> UField;
}

/// A finite field over a prime that uses Montgomery modular arithmetic
/// to increase the performance of the multiplication operator.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct FiniteField<UField> {
    p: UField,
    m: UField,
    one: FiniteFieldElement<UField>,
}

impl FiniteField<u32> {
    /// Returns the unit element in Montgomory form, ie.e 1 + 2^32 mod a.
    fn get_one(a: u32) -> u32 {
        if a as u64 <= 1u64 << 31 {
            let res = (((1u64 << 31) % a as u64) << 1) as u32;

            if res < a {
                res
            } else {
                res - a
            }
        } else {
            a.wrapping_neg()
        }
    }

    /// Returns -a^-1 mod 2^32.
    fn inv_2_32(a: u32) -> u32 {
        let mut ret: u32 = HENSEL_LIFTING_MASK[((a >> 1) & 127) as usize] as u32;
        ret = ret.wrapping_mul(a.wrapping_mul(ret).wrapping_add(2));
        ret = ret.wrapping_mul(a.wrapping_mul(ret).wrapping_add(2));
        ret
    }
}

impl FiniteFieldWorkspace for u32 {
    fn to_u64(&self) -> u64 {
        *self as u64
    }
}

impl FiniteFieldCore<u32> for FiniteField<u32> {
    /// Create a new finite field. `n` must be a prime larger than 2.
    fn new(p: u32) -> FiniteField<u32> {
        assert!(p % 2 != 0);

        Self {
            p,
            m: Self::inv_2_32(p),
            one: FiniteFieldElement(Self::get_one(p)),
        }
    }

    fn get_prime(&self) -> u32 {
        self.p
    }

    /// Convert a number in a prime field a % n to Montgomory form.
    #[inline(always)]
    fn to_element(&self, a: u32) -> FiniteFieldElement<u32> {
        // TODO: slow, faster alternatives may need assembly
        FiniteFieldElement((((a as u64) << 32) % self.p as u64) as u32)
    }

    /// Convert a number from Montgomory form to standard form.
    #[inline(always)]
    fn from_element(&self, a: FiniteFieldElement<u32>) -> u32 {
        self.mul(&a, &FiniteFieldElement(1)).0
    }
}

impl Ring for FiniteField<u32> {
    type Element = FiniteFieldElement<u32>;

    /// Add two numbers in Montgomory form.
    #[inline(always)]
    fn add(&self, a: &Self::Element, b: &Self::Element) -> Self::Element {
        let mut t = a.0 as u64 + b.0 as u64;

        if t >= self.p as u64 {
            t -= self.p as u64;
        }

        FiniteFieldElement(t as u32)
    }

    /// Subtract `b` from `a`, where `a` and `b` are in Montgomory form.
    #[inline(always)]
    fn sub(&self, a: &Self::Element, b: &Self::Element) -> Self::Element {
        FiniteFieldElement(if a.0 >= b.0 {
            a.0 - b.0
        } else {
            a.0 + (self.p - b.0)
        })
    }

    /// Multiply two numbers in Montgomory form.
    #[inline(always)]
    fn mul(&self, a: &Self::Element, b: &Self::Element) -> Self::Element {
        let t = a.0 as u64 * b.0 as u64;
        let m = (t as u32).wrapping_mul(self.m);
        let u = ((t.wrapping_add(m as u64 * self.p as u64)) >> 32) as u32;

        // correct for overflow
        if u < (t >> 32) as u32 {
            return FiniteFieldElement(u.wrapping_sub(self.p));
        }

        FiniteFieldElement(if u >= self.p { u - self.p } else { u })
    }

    #[inline(always)]
    fn add_assign(&self, a: &mut Self::Element, b: &Self::Element) {
        *a = self.add(a, b);
    }

    #[inline(always)]
    fn sub_assign(&self, a: &mut Self::Element, b: &Self::Element) {
        *a = self.sub(a, b);
    }

    #[inline(always)]
    fn mul_assign(&self, a: &mut Self::Element, b: &Self::Element) {
        *a = self.mul(a, b);
    }

    fn add_mul_assign(&self, a: &mut Self::Element, b: &Self::Element, c: &Self::Element) {
        self.add_assign(a, &self.mul(b, c));
    }

    fn sub_mul_assign(&self, a: &mut Self::Element, b: &Self::Element, c: &Self::Element) {
        self.sub_assign(a, &self.mul(b, c));
    }

    /// Computes -x mod n.
    #[inline]
    fn neg(&self, a: &Self::Element) -> Self::Element {
        FiniteFieldElement(self.p - a.0)
    }

    #[inline]
    fn zero(&self) -> Self::Element {
        FiniteFieldElement(0)
    }

    /// Return the unit element in Montgomory form.
    #[inline]
    fn one(&self) -> Self::Element {
        self.one
    }

    /// Compute b^e % n.
    #[inline]
    fn pow(&self, b: &Self::Element, mut e: u64) -> Self::Element {
        let mut b = *b;
        let mut x = self.one();
        while e != 0 {
            if e & 1 != 0 {
                x = self.mul(&x, &b);
            }
            b = self.mul(&b, &b);
            e /= 2;
        }

        x
    }

    #[inline]
    fn is_zero(a: &Self::Element) -> bool {
        a.0 == 0
    }

    #[inline]
    fn is_one(&self, a: &Self::Element) -> bool {
        a == &self.one
    }

    #[inline]
    fn get_unit(&self, a: &Self::Element) -> Self::Element {
        *a
    }

    #[inline]
    fn get_inv_unit(&self, a: &Self::Element) -> Self::Element {
        self.inv(a)
    }

    fn sample(&self, rng: &mut impl rand::RngCore, range: (i64, i64)) -> Self::Element {
        let r = rng.gen_range(range.0.max(0)..range.1.min(self.p as i64));
        FiniteFieldElement(r as u32)
    }

    fn fmt_display(&self, element: &Self::Element, f: &mut Formatter<'_>) -> Result<(), Error> {
        if f.sign_plus() {
            write!(f, "+{}", self.from_element(*element))
        } else {
            write!(f, "{}", self.from_element(*element))
        }
    }
}

impl EuclideanDomain for FiniteField<u32> {
    #[inline]
    fn rem(&self, _: &Self::Element, _: &Self::Element) -> Self::Element {
        FiniteFieldElement(0)
    }

    #[inline]
    fn quot_rem(&self, a: &Self::Element, b: &Self::Element) -> (Self::Element, Self::Element) {
        (self.mul(a, &self.inv(b)), FiniteFieldElement(0))
    }

    #[inline]
    fn gcd(&self, _: &Self::Element, _: &Self::Element) -> Self::Element {
        self.one()
    }
}

impl Field for FiniteField<u32> {
    #[inline]
    fn div(&self, a: &Self::Element, b: &Self::Element) -> Self::Element {
        self.mul(a, &self.inv(b))
    }

    #[inline]
    fn div_assign(&self, a: &mut Self::Element, b: &Self::Element) {
        *a = self.mul(a, &self.inv(b));
    }

    /// Computes x^-1 mod n.
    fn inv(&self, a: &Self::Element) -> Self::Element {
        assert!(a.0 != 0, "0 is not invertible");

        // apply multiplication with 1 twice to get the correct scaling of R=2^32
        // see the paper [Montgomery Arithmetic from a Software Perspective](https://eprint.iacr.org/2017/1057.pdf).
        let x_mont = self
            .mul(&self.mul(a, &FiniteFieldElement(1)), &FiniteFieldElement(1))
            .0;

        // extended Euclidean algorithm: a x + b p = gcd(x, p) = 1 or a x = 1 (mod p)
        let mut u1: u32 = 1;
        let mut u3 = x_mont;
        let mut v1: u32 = 0;
        let mut v3 = self.p;
        let mut even_iter: bool = true;

        while v3 != 0 {
            let q = u3 / v3;
            let t3 = u3 % v3;
            let t1 = u1 + q * v1;
            u1 = v1;
            v1 = t1;
            u3 = v3;
            v3 = t3;
            even_iter = !even_iter;
        }

        debug_assert!(u3 == 1);
        if even_iter {
            FiniteFieldElement(u1)
        } else {
            FiniteFieldElement(self.p - u1)
        }
    }
}

impl FiniteFieldWorkspace for u64 {
    #[inline]
    fn to_u64(&self) -> u64 {
        *self
    }
}

impl FiniteField<u64> {
    /// Returns the unit element in Montgomory form, ie.e 1 + 2^64 mod a.
    fn get_one(a: u64) -> u64 {
        if a as u128 <= 1u128 << 63 {
            let res = (((1u128 << 63) % a as u128) << 1) as u64;

            if res < a {
                res
            } else {
                res - a
            }
        } else {
            a.wrapping_neg()
        }
    }

    /// Returns -a^-1 mod 2^64.
    fn inv_2_64(a: u64) -> u64 {
        let mut ret: u64 = HENSEL_LIFTING_MASK[((a >> 1) & 127) as usize] as u64;
        ret = ret.wrapping_mul(a.wrapping_mul(ret).wrapping_add(2));
        ret = ret.wrapping_mul(a.wrapping_mul(ret).wrapping_add(2));
        ret = ret.wrapping_mul(a.wrapping_mul(ret).wrapping_add(2));
        ret
    }
}

impl FiniteFieldCore<u64> for FiniteField<u64> {
    /// Create a new finite field. `n` must be a prime larger than 2.
    fn new(p: u64) -> FiniteField<u64> {
        assert!(p % 2 != 0);

        FiniteField {
            p,
            m: Self::inv_2_64(p),
            one: FiniteFieldElement(Self::get_one(p)),
        }
    }

    fn get_prime(&self) -> u64 {
        self.p
    }

    /// Convert a number in a prime field a % n to Montgomory form.
    #[inline(always)]
    fn to_element(&self, a: u64) -> FiniteFieldElement<u64> {
        // TODO: slow, faster alternatives may need assembly
        FiniteFieldElement((((a as u128) << 64) % self.p as u128) as u64)
    }

    /// Convert a number from Montgomory form to standard form.
    #[inline(always)]
    fn from_element(&self, a: FiniteFieldElement<u64>) -> u64 {
        self.mul(&a, &FiniteFieldElement(1)).0
    }
}

impl<UField: Display> Display for FiniteField<UField> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, " % {}", self.p)
    }
}

impl Ring for FiniteField<u64> {
    type Element = FiniteFieldElement<u64>;

    /// Add two numbers in Montgomory form.
    #[inline(always)]
    fn add(&self, a: &Self::Element, b: &Self::Element) -> Self::Element {
        let mut t = a.0 as u128 + b.0 as u128;

        if t >= self.p as u128 {
            t -= self.p as u128;
        }

        FiniteFieldElement(t as u64)
    }

    /// Subtract `b` from `a`, where `a` and `b` are in Montgomory form.
    #[inline(always)]
    fn sub(&self, a: &Self::Element, b: &Self::Element) -> Self::Element {
        if a.0 >= b.0 {
            FiniteFieldElement(a.0 - b.0)
        } else {
            FiniteFieldElement(a.0 + (self.p - b.0))
        }
    }

    /// Multiply two numbers in Montgomory form.
    #[inline(always)]
    fn mul(&self, a: &Self::Element, b: &Self::Element) -> Self::Element {
        let t = a.0 as u128 * b.0 as u128;
        let m = (t as u64).wrapping_mul(self.m);
        let u = ((t.wrapping_add(m as u128 * self.p as u128)) >> 64) as u64;

        // correct for overflow
        if u < (t >> 64) as u64 {
            return FiniteFieldElement(u.wrapping_sub(self.p));
        }

        if u >= self.p {
            FiniteFieldElement(u - self.p)
        } else {
            FiniteFieldElement(u)
        }
    }

    #[inline]
    fn add_assign(&self, a: &mut Self::Element, b: &Self::Element) {
        *a = self.add(a, b);
    }

    #[inline]
    fn sub_assign(&self, a: &mut Self::Element, b: &Self::Element) {
        *a = self.sub(a, b);
    }

    #[inline]
    fn mul_assign(&self, a: &mut Self::Element, b: &Self::Element) {
        *a = self.mul(a, b);
    }

    fn add_mul_assign(&self, a: &mut Self::Element, b: &Self::Element, c: &Self::Element) {
        self.add_assign(a, &self.mul(b, c));
    }

    fn sub_mul_assign(&self, a: &mut Self::Element, b: &Self::Element, c: &Self::Element) {
        self.sub_assign(a, &self.mul(b, c));
    }

    /// Computes -x mod n.
    #[inline]
    fn neg(&self, a: &Self::Element) -> Self::Element {
        FiniteFieldElement(self.p - a.0)
    }

    #[inline]
    fn zero(&self) -> Self::Element {
        FiniteFieldElement(0)
    }

    /// Return the unit element in Montgomory form.
    #[inline]
    fn one(&self) -> Self::Element {
        self.one
    }

    /// Compute b^e % n.
    #[inline]
    fn pow(&self, b: &Self::Element, mut e: u64) -> Self::Element {
        let mut b = *b;
        let mut x = self.one();
        while e != 0 {
            if e & 1 != 0 {
                x = self.mul(&x, &b);
            }
            b = self.mul(&b, &b);
            e /= 2;
        }

        x
    }

    #[inline]
    fn is_zero(a: &Self::Element) -> bool {
        a.0 == 0
    }

    #[inline]
    fn is_one(&self, a: &Self::Element) -> bool {
        a == &self.one
    }

    #[inline]
    fn get_unit(&self, a: &Self::Element) -> Self::Element {
        *a
    }

    #[inline]
    fn get_inv_unit(&self, a: &Self::Element) -> Self::Element {
        self.inv(a)
    }

    fn sample(&self, rng: &mut impl rand::RngCore, range: (i64, i64)) -> Self::Element {
        let r = rng.gen_range(range.0.max(0)..range.1.min(self.p.min(i64::MAX as u64) as i64));
        FiniteFieldElement(r as u64)
    }

    fn fmt_display(&self, element: &Self::Element, f: &mut Formatter<'_>) -> Result<(), Error> {
        if f.sign_plus() {
            write!(f, "+{}", self.from_element(*element))
        } else {
            write!(f, "{}", self.from_element(*element))
        }
    }
}

impl EuclideanDomain for FiniteField<u64> {
    #[inline]
    fn rem(&self, _: &Self::Element, _: &Self::Element) -> Self::Element {
        FiniteFieldElement(0)
    }

    #[inline]
    fn quot_rem(&self, a: &Self::Element, b: &Self::Element) -> (Self::Element, Self::Element) {
        (self.mul(a, &self.inv(b)), FiniteFieldElement(0))
    }

    #[inline]
    fn gcd(&self, _: &Self::Element, _: &Self::Element) -> Self::Element {
        self.one()
    }
}

impl Field for FiniteField<u64> {
    #[inline]
    fn div(&self, a: &Self::Element, b: &Self::Element) -> Self::Element {
        self.mul(a, &self.inv(b))
    }

    #[inline]
    fn div_assign(&self, a: &mut Self::Element, b: &Self::Element) {
        *a = self.mul(a, &self.inv(b));
    }

    /// Computes x^-1 mod n.
    fn inv(&self, a: &Self::Element) -> Self::Element {
        assert!(a.0 != 0, "0 is not invertible");

        // apply multiplication with 1 twice to get the correct scaling of R=2^64
        // see the paper [Montgomery Arithmetic from a Software Perspective](https://eprint.iacr.org/2017/1057.pdf).
        let x_mont = self
            .mul(&self.mul(a, &FiniteFieldElement(1)), &FiniteFieldElement(1))
            .0;

        // extended Euclidean algorithm: a x + b p = gcd(x, p) = 1 or a x = 1 (mod p)
        let mut u1: u64 = 1;
        let mut u3 = x_mont;
        let mut v1: u64 = 0;
        let mut v3 = self.p;
        let mut even_iter: bool = true;

        while v3 != 0 {
            let q = u3 / v3;
            let t3 = u3 % v3;
            let t1 = u1 + q * v1;
            u1 = v1;
            v1 = t1;
            u3 = v3;
            v3 = t3;
            even_iter = !even_iter;
        }

        debug_assert!(u3 == 1);
        FiniteFieldElement(if even_iter { u1 } else { self.p - u1 })
    }
}

/// Do a deterministic Miller test to check if `n` is a prime.
/// Since `n` is a `u64`, a basis of only 7 witnesses has to be tested.
///
/// Based on [Wojciech Izykowski's implementation](https://github.com/wizykowski/miller-rabin).
pub fn is_prime_u64(n: u64) -> bool {
    // shortest SPRP basis from Jim Sinclair for testing primality of u64
    let witnesses: [u64; 7] = [2, 325, 9375, 28178, 450775, 9780504, 1795265022];

    if n < 2 {
        return false;
    }

    if n % 2 == 0 {
        return n == 2;
    }

    let mut s = 0;
    let mut d = n - 1;
    while d % 2 == 0 {
        d /= 2;
        s += 1;
    }

    let f = FiniteField::<u64>::new(n);
    let neg_one = FiniteFieldElement(n.wrapping_sub(f.one().0));

    'test: for a in witnesses {
        let a = f.to_element(a);

        if a.0 == 0 {
            continue;
        }

        let mut x = f.pow(&a, d);

        if x == f.one() || x == neg_one {
            continue;
        }

        for _ in 0..s {
            x = f.mul(&x, &x);

            if x == f.one() {
                return false;
            }
            if x == neg_one {
                continue 'test;
            }
        }

        return false;
    }

    true
}

/// An interator over consecutive 64-bit primes.
pub struct PrimeIteratorU64 {
    current_number: u64,
}

impl PrimeIteratorU64 {
    /// Create a new prime iterator that is larger than `start`.
    pub fn new(start: u64) -> Self {
        Self {
            current_number: start.max(1),
        }
    }
}

impl Iterator for PrimeIteratorU64 {
    type Item = u64;

    /// Yield the next prime or `None` if `u64::MAX` has been reached.
    fn next(&mut self) -> Option<u64> {
        while self.current_number < u64::MAX {
            self.current_number += 1;

            if is_prime_u64(self.current_number) {
                return Some(self.current_number);
            }
        }

        None
    }
}
