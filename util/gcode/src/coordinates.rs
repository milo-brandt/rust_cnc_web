use std::ops::{Neg, Mul};

#[derive(Debug, Clone, PartialEq)]
pub struct Position(pub Vec<f64>);
#[derive(Debug, Clone, PartialEq)]
pub struct Offset(pub Vec<f64>);

#[derive(Debug, Clone, PartialEq)]
pub struct PartialPosition(pub Vec<Option<f64>>);
impl PartialPosition {
    pub fn update_from(&mut self, other: &PartialPosition) {
        for (current, new) in self.0.iter_mut().zip(other.0.iter()) {
            if let Some(v) = new {
                *current = Some(*v);
            }
        }
    }
    pub fn or(mut self, other: &PartialPosition) -> Self {
        self.update_from(other);
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartialOffset(pub Vec<Option<f64>>);

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Sign { Positive, Negative }
impl Sign {
    pub fn apply<T: Neg<Output = T>>(self, value: T) -> T {
        match self {
            Sign::Positive => value,
            Sign::Negative => -value,
        }
    }
}
impl Mul<Sign> for Sign {
    type Output = Sign;

    fn mul(self, rhs: Sign) -> Self::Output {
        match (self, rhs) {
            (Sign::Positive, Sign::Positive) => Sign::Positive,
            (Sign::Positive, Sign::Negative) => Sign::Negative,
            (Sign::Negative, Sign::Positive) => Sign::Negative,
            (Sign::Negative, Sign::Negative) => Sign::Positive,
        }
    }
}
impl Neg for Sign {
    type Output = Sign;

    fn neg(self) -> Self::Output {
        match self {
            Sign::Positive => Sign::Negative,
            Sign::Negative => Sign::Positive,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ArcPlane(pub u8, pub u8);
impl ArcPlane {
    pub fn compare(lhs: &ArcPlane, rhs: &ArcPlane) -> Option<Sign> {
        if lhs.0 == rhs.0 && lhs.1 == rhs.1 {
            Some(Sign::Positive)
        } else if lhs.1 == rhs.0 && lhs.0 == rhs.1 {
            Some(Sign::Negative)
        } else {
            None
        }
    }
}


