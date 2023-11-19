use std::{cmp::Ordering, convert::Infallible};

use crate::coordinates::PartialPosition;

pub struct DepthMapPoint {
    pub x: f64,
    pub y: f64,
    pub depth_offset: f64,
}


#[derive(PartialEq,PartialOrd)]
struct NonNan(f64);

impl NonNan {
    fn new(val: f64) -> Option<NonNan> {
        if val.is_nan() {
            None
        } else {
            Some(NonNan(val))
        }
    }
}

impl Eq for NonNan {}

impl Ord for NonNan {
    fn cmp(&self, other: &NonNan) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}
fn square(x: f64) -> f64 { x * x }
// should be non-empty
pub fn depth_map_to_transformer(points: Vec<DepthMapPoint>) -> impl Fn(PartialPosition) -> Result<PartialPosition, Infallible> {
    let max_height = points
        .iter()
        .map(|pt| NonNan::new(pt.depth_offset).expect("Depth map should not include NaN"))
        .max()
        .expect("Depth map points should not be empty.");
    move |mut position: PartialPosition| {
        assert!(position.0.len() >= 3);
        if position.0[2].is_some() {
            let z_offset = if let (Some(x), Some(y)) = (position.0[0], position.0[1]) {
                points
                    .iter()
                    .min_by_key(|pt| NonNan::new(
                        square(pt.x - x) + square(pt.y - y) 
                    ).unwrap())
                    .unwrap()
                    .depth_offset
            } else {
                max_height.0
            };
            *position.0[2].as_mut().unwrap() += z_offset;
        };
        Ok(position)
    }
}