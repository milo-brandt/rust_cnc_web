use std::{ops::{Neg, Mul}, convert::Infallible};

use crate::coordinates::{Position, PartialPosition, PartialOffset, Offset};

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

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SignedIndex(pub Sign, pub u8);
/// A transformation obtained by permuting coordinates and flipping the sign on some;
/// 
/// In SimpleTransform(permutation), if permutation[0] = (sign, i) then (1, 0, 0, ...) maps to (..., 0, sign, 0, ...)
/// where sign occurs at the ith index.
pub struct SimpleTransform {
    pub permutation: Vec<SignedIndex>,
    pub offset: Offset
}
impl SimpleTransform {
    pub fn is_translation(&self) -> bool {
        self.permutation.iter().enumerate().all(|(index, signed_index)| signed_index == &SignedIndex(Sign::Positive, index as u8))
    }
}
pub trait Transform<T> {
    fn transform(&self, value: &T) -> T;
}
pub trait TryTransform<T> {
    type Error;
    fn try_transform(&self, value: &T) -> Result<T, Self::Error>;
}
impl<T, S: Transform<T>> TryTransform<T> for S {
    type Error = Infallible;
    fn try_transform(&self, value: &T) -> Result<T, Self::Error> {
        Ok(self.transform(value))
    }
}

impl Transform<Position> for SimpleTransform {
    fn transform(&self, value: &Position) -> Position {
        let mut results = Position(self.offset.0.clone());
        for (SignedIndex(sign, index), original) in self.permutation.iter().zip(value.0.iter()) {
            results.0[*index as usize] += sign.apply(*original)
        }
        results
    }
}
impl Transform<PartialPosition> for SimpleTransform {
    fn transform(&self, value: &PartialPosition) -> PartialPosition {
        let mut results = PartialPosition(Vec::new());
        results.0.resize(self.offset.0.len(), None);
        for (SignedIndex(sign, index), original) in self.permutation.iter().zip(value.0.iter()) {
            results.0[*index as usize] = original.map(|v| self.offset.0[*index as usize] + sign.apply(v));
        }
        results
    }
}
impl Transform<Offset> for SimpleTransform {
    fn transform(&self, value: &Offset) -> Offset {
        let mut results = Offset(Vec::new());
        results.0.resize(self.offset.0.len(), 0.0);
        for (SignedIndex(sign, index), original) in self.permutation.iter().zip(value.0.iter()) {
            results.0[*index as usize] = sign.apply(*original)
        }
        results
    }
}
impl Transform<PartialOffset> for SimpleTransform {
    fn transform(&self, value: &PartialOffset) -> PartialOffset {
        let mut results = PartialOffset(Vec::new());
        results.0.resize(self.offset.0.len(), None);
        for (SignedIndex(sign, index), original) in self.permutation.iter().zip(value.0.iter()) {
            results.0[*index as usize] = original.map(|v| self.offset.0[*index as usize] + sign.apply(v));
        }
        results
    }
}