// use std::{ops::Neg, mem::swap};

// use crate::coordinates::Position;

// // #[derive(Copy, Clone, PartialEq, Eq, Debug)]
// // pub enum ProbeOptionality { Optional, Mandatory }
// // #[derive(Copy, Clone, PartialEq, Eq, Debug)]
// // pub enum ProbeDirection { Towards, Away }
// // #[derive(Copy, Clone, PartialEq, Eq, Debug)]
// // pub struct ProbeMode(ProbeDirection, ProbeOptionality);

// pub struct LinearMotion {
//     pub target: Position,
// }
// pub struct HelicalMotion {
//     // The projection of position -> (position[principal_axis], position[secondary_axis]) will trace a
//     // counterclockwise circle around (principal_center, secondary_center)
//     // maybe really - principal_center is optional...
//     pub principal_axis: u8,
//     pub principal_center: f64,
//     pub secondary_axis: u8,
//     pub secondary_center: f64,
//     pub angle: f64, // how far to travel; mostly to determine number of revolutions.
//     pub target: Position,
// }
// pub enum Motion {
//     LinearMotion(LinearMotion),
//     HelicalMotion(HelicalMotion),
// }

// #[derive(Copy, Clone, PartialEq, Eq, Debug)]
// pub enum Sign { Positive, Negative }
// impl Sign {
//     pub fn apply<T: Neg<Output = T>>(self, value: T) -> T {
//         match self {
//             Sign::Positive => value,
//             Sign::Negative => -value,
//         }
//     }
// }
// #[derive(Copy, Clone, PartialEq, Eq, Debug)]
// pub struct SignedIndex(pub Sign, pub u8);
// /// A transformation obtained by permuting coordinates and flipping the sign on some;
// /// 
// /// In SimpleTransform(permutation), if permutation[0] = (sign, i) then (1, 0, 0, ...) maps to (..., 0, sign, 0, ...)
// /// where sign occurs at the ith index.
// pub struct SimpleTransform {
//     pub permutation: Vec<SignedIndex>,
//     pub offset: Position
// }
// trait Transform<T> {
//     fn transform(&self, value: &T) -> T;
// }
// impl Transform<LinearMotion> for SimpleTransform {
//     fn transform(&self, value: &LinearMotion) -> LinearMotion {
//         LinearMotion {
//             target: self.transform(&value.target),
//         }
//     }
// }
// impl Transform<HelicalMotion> for SimpleTransform {
//     fn transform(&self, value: &HelicalMotion) -> HelicalMotion {
//         let SignedIndex(principal_sign, mut principal_axis) = self.permutation[value.principal_axis as usize];
//         let SignedIndex(secondary_sign, mut secondary_axis) = self.permutation[value.secondary_axis as usize];
//         let mut principal_center = self.offset.0[principal_axis as usize] + principal_sign.apply(value.principal_center);
//         let mut secondary_center = self.offset.0[secondary_axis as usize] + secondary_sign.apply(value.secondary_center);
//         if principal_sign != secondary_sign {
//             // In this case, the projection to the circle will ultimately be flipped and therefore clockwise; to represent this,
//             // swap the axes, so that the projection is again counterclockwise.
//             swap(&mut principal_center, &mut secondary_center);
//             swap(&mut principal_axis, &mut secondary_axis);
//         }
//         HelicalMotion {
//             principal_axis,
//             principal_center,
//             secondary_axis,
//             secondary_center,
//             angle: value.angle,
//             target: self.transform(&value.target),
//         }
//     }
// }
// impl<T: Transform<HelicalMotion> + Transform<LinearMotion>> Transform<Motion> for T {
//     fn transform(&self, value: &Motion) -> Motion {
//         match value {
//             Motion::LinearMotion(motion) => Motion::LinearMotion(self.transform(motion)),
//             Motion::HelicalMotion(motion) => Motion::HelicalMotion(self.transform(motion)),
//         }
//     }
// }