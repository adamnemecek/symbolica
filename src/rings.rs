pub mod finite_field;
pub mod integer;
pub mod linear_system;
pub mod rational;
pub mod rational_polynomial;

use std::fmt::{Debug, Display, Error, Formatter};

pub trait Ring: Clone + Copy + PartialEq + Debug + Display {
    type Element: Clone + PartialEq + Debug;

    fn add(&self, a: &Self::Element, b: &Self::Element) -> Self::Element;
    fn sub(&self, a: &Self::Element, b: &Self::Element) -> Self::Element;
    fn mul(&self, a: &Self::Element, b: &Self::Element) -> Self::Element;
    fn add_assign(&self, a: &mut Self::Element, b: &Self::Element);
    fn sub_assign(&self, a: &mut Self::Element, b: &Self::Element);
    fn mul_assign(&self, a: &mut Self::Element, b: &Self::Element);
    fn add_mul_assign(&self, a: &mut Self::Element, b: &Self::Element, c: &Self::Element);
    fn sub_mul_assign(&self, a: &mut Self::Element, b: &Self::Element, c: &Self::Element);
    fn neg(&self, a: &Self::Element) -> Self::Element;
    fn zero(&self) -> Self::Element;
    fn one(&self) -> Self::Element;
    fn pow(&self, b: &Self::Element, e: u64) -> Self::Element;
    fn is_zero(a: &Self::Element) -> bool;
    fn is_one(&self, a: &Self::Element) -> bool;
    fn get_unit(&self, a: &Self::Element) -> Self::Element;
    fn get_inv_unit(&self, a: &Self::Element) -> Self::Element;

    fn sample(&self, rng: &mut impl rand::RngCore, range: (i64, i64)) -> Self::Element;
    fn fmt_display(&self, element: &Self::Element, f: &mut Formatter<'_>) -> Result<(), Error>;
}

pub trait EuclideanDomain: Ring {
    fn rem(&self, a: &Self::Element, b: &Self::Element) -> Self::Element;
    fn quot_rem(&self, a: &Self::Element, b: &Self::Element) -> (Self::Element, Self::Element);
    fn gcd(&self, a: &Self::Element, b: &Self::Element) -> Self::Element;
}

pub trait Field: EuclideanDomain {
    fn div(&self, a: &Self::Element, b: &Self::Element) -> Self::Element;
    fn div_assign(&self, a: &mut Self::Element, b: &Self::Element);
    fn inv(&self, a: &Self::Element) -> Self::Element;
}

pub struct RingPrinter<'a, R: Ring> {
    pub ring: &'a R,
    pub element: &'a R::Element,
}

impl<'a, R: Ring> Display for RingPrinter<'a, R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.ring.fmt_display(self.element, f)
    }
}
