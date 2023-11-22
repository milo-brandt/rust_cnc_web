use itertools::Either;

use crate::{gcode::{MachineState, Line, CommandContent, LinearMove, HelicalMove, ModalUpdates, MotionMode}, coordinates::{ArcPlane, PartialPosition}};

#[derive(Copy, Clone, Debug)]
pub enum TagError {
    Unsupported,
    UnknownPosition,
    MissingFeedrate,
}
#[derive(Clone, Debug)]
pub struct Tag {
    position: (f64, f64),
    minimum_height: f64,
    radius: f64,
}
pub struct TagApplier {
    is_compensated: bool,
    vertical_feedrate: Option<f64>,
    tag: Tag,
}
#[derive(Clone, Copy, Debug)]
struct Interval(Option<(f64, f64)>);
struct QuadraticSegment {
    a: f64, // must be > 0
    b: f64,
    c: f64,
}
impl Interval {
    fn empty() -> Self {
        Self(None)
    }
    fn contains(self, value: f64) -> bool {
        match self.0 {
            None => false,
            Some((min, max)) => min <= value && value <= max,
        }
    }
    fn intersect(self, other: Self) -> Self {
        match (self.0, other.0) {
            (Some((lhs_min, lhs_max)), Some((rhs_min, rhs_max))) => {
                let min = f64::max(lhs_min, rhs_min);
                let max = f64::min(lhs_max, rhs_max);
                if min >= max {
                    Self(None)
                } else {
                    Self(Some((min, max)))
                }
            }
            _ => Self(None)
        }
    }
    fn from_endpoints(a: f64, b: f64) -> Self {
        if a < b { 
            Self(Some((a, b)))
        } else {
            Self(Some((b, a)))
        }
    }
    /// Given two (x, y) pairs, calculate the interval of x for the segment between them upon which
    /// lays about threshold.
    fn portion_of_segment_below_threshold(start_point: (f64, f64), end_point: (f64, f64), threshold: f64) -> Self {
        let intersection_point = start_point.0 + (end_point.0 - start_point.0) * (threshold - start_point.1) / (end_point.1 - start_point.1);
        match (start_point.1 < threshold, end_point.1 < threshold) {
            (true, true) => Self::from_endpoints(start_point.0, end_point.0),
            (true, false) => Self::from_endpoints(start_point.0, intersection_point),
            (false, true) => Self::from_endpoints(intersection_point, end_point.0),
            (false, false) => Self(None),
        }
    }
    fn negative_portion_of_parabola(quadratic_segment: QuadraticSegment) -> Self {
        let vertex = -quadratic_segment.b / (2.0 * quadratic_segment.a);
        let discriminant = quadratic_segment.b * quadratic_segment.b - 4.0 * quadratic_segment.a * quadratic_segment.c;
        if discriminant <= 0.0 {
            return Self::empty();
        }
        let root_distance = f64::sqrt(discriminant) / (2.0 * quadratic_segment.a); 
        Self(Some((vertex - root_distance, vertex + root_distance)))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, derive_more::Add, derive_more::Sub, derive_more::Mul)]
struct ArcVector(f64, f64);
impl ArcVector {
    pub fn dot(self, other: Self) -> f64 {
        self.0 * other.0 + self.1 * other.1
    }
    pub fn magnitude_squared(self) -> f64 {
        self.dot(self)
    }
}

// end should have more items set than start.
fn bad_progress_interval(start: PartialPosition, end: PartialPosition, tag: &Tag) -> Result<Interval, TagError> {
    if start.0[0] == end.0[0] && start.0[1] == end.0[1] && start.0[2].map_or(true, |z| z >= tag.minimum_height) && end.0[2].map_or(true, |z| z >= tag.minimum_height) {
        // Any vertical-only move is ok as long as we either don't know the height or are above the danger height.
        return Ok(Interval::empty());
    } else if start.0[2].is_some_and(|z| z >= tag.minimum_height) && end.0[2].is_some_and(|z| z >= tag.minimum_height) {
        // If we know the heights and they are above the danger heights, everything is okay.
        return Ok(Interval::empty());
    } else if start.0[0].is_none() || start.0[1].is_none() {
        // If we pass below the danger height and don't know our horizontal position, we cannot be sure tagging is safe.
        return Err(TagError::UnknownPosition);
    }
    let below_interval = Interval::portion_of_segment_below_threshold(
        (0.0, start.0[2].unwrap()),
        (1.0, end.0[2].unwrap()),
        tag.minimum_height
    );
    let valid_interval = Interval::from_endpoints(0.0, 1.0);
    let start = ArcVector(start.0[0].unwrap(), start.0[1].unwrap());
    let end = ArcVector(end.0[0].unwrap(), end.0[1].unwrap());
    let direction = end - start;
    let offset = start - ArcVector(tag.position.0, tag.position.1); 
    let radius_squared = tag.radius * tag.radius;
    // Work out the quadratic for ||start - center + t * direction||^2 = radius^2; various hacks for slightly better 
    // numerical stability.
    let close_interval = {
        if offset.magnitude_squared() <= radius_squared && (offset + direction).magnitude_squared() <= radius_squared {
            Interval::from_endpoints(0.0, 1.0)
        } else {
            let a = direction.dot(direction);
            let b = 2.0 * direction.dot(offset);
            let c = offset.dot(offset) - radius_squared;
            if c > 0.0 && c > a.abs() + b.abs() {
                Interval::empty()
            } else {
                Interval::negative_portion_of_parabola(QuadraticSegment { a, b, c })
            }
        }
    };
    Ok(valid_interval.intersect(below_interval).intersect(close_interval))
}

enum PositionTag {
    Up(f64),
    Down(f64)
}
impl TagApplier {
    //Tags should be sorted by depth...
    pub fn new(tag: Tag, vertical_feedrate: Option<f64>) -> Self {
        Self {
            is_compensated: false,
            vertical_feedrate,
            tag,
        }
    }
    // pre_machine_state of the code being _input_ not the code being _output_.
    pub fn apply_to(&mut self, pre_machine_state: &MachineState, line: Line) -> Result<impl Iterator<Item=Line>, TagError> {
        match &line.command {
            Some(CommandContent::LinearMove(LinearMove(target))) => {
                let position_at_time = |t: f64| if t == 1.0 {
                    target.clone()
                } else if t == 0.0 {
                    let mut position = PartialPosition::empty(target.0.len() as u8);
                    position.0[2] = pre_machine_state.position.0[2];
                    position
                } else {
                    PartialPosition(
                        pre_machine_state.position.0.iter().zip(target.0.iter()).map(|(before, target)| match (before, target) {
                            (Some(before), target) => Some(*before * (1.0 - t) + target.unwrap_or(*before) * t),
                            _ => None
                        }).collect()
                    )
                };
                let up_position_at_time = |t: f64| {
                    let mut position = position_at_time(t);
                    position.0[2] = Some(self.tag.minimum_height);
                    position
                };

                let bad_interval = bad_progress_interval(pre_machine_state.position.clone(), target.clone(), &self.tag)?;
                let mut lines = Vec::new();
                let original_feedrate = line.modal_updates.feedrate.or(pre_machine_state.feedrate);
                let original_mode = line.modal_updates.motion_mode.or(pre_machine_state.motion_mode);
                let mut modal_updates = Some(line.modal_updates);
                let mut get_original_modal_updates = || {
                    match modal_updates.take() {
                        Some(mut modal_updates) => {
                            if self.is_compensated && self.vertical_feedrate.is_some() && modal_updates.feedrate.is_none() {
                                if original_feedrate.is_some() {
                                    modal_updates.feedrate = original_feedrate;
                                } else {
                                    return Err(TagError::MissingFeedrate);
                                }
                            }
                            Ok(modal_updates)
                        },
                        None => {
                            if self.vertical_feedrate.is_some() {
                                match original_feedrate {
                                    Some(feedrate) => {
                                        let mut updates = ModalUpdates::default();
                                        updates.feedrate = Some(feedrate);
                                        updates.motion_mode = original_mode; // TODO: Track mode?
                                        Ok(updates)
                                    }
                                    // We've overridden the feedrate but don't know the original
                                    None => Err(TagError::MissingFeedrate)
                                }
                            } else {
                                let mut updates = ModalUpdates::default();
                                updates.motion_mode = original_mode;
                                Ok(updates)
                            }
                        }
                    }
                };
                let get_vertical_modal_updates = || ModalUpdates {
                    feedrate: self.vertical_feedrate,
                    motion_mode: Some(MotionMode::Controlled),
                    coordinate_mode: None,
                    units: None,
                    arc_plane: None,
                    coordinate_system: None,
                };
                match bad_interval.0 {
                    Some((min, max)) => {
                        // Get to the right start position if not already there.
                        if self.is_compensated {
                            if min > 0.0 {
                                // Travel down to the actual path.
                                lines.push(Line {
                                    modal_updates: get_vertical_modal_updates(),
                                    command: Some(CommandContent::LinearMove(LinearMove(position_at_time(0.0))))
                                });
                            }
                        } else {
                            if min == 0.0 {
                                // Travel up to the compensated path
                                lines.push(Line {
                                    modal_updates: get_vertical_modal_updates(),
                                    command: Some(CommandContent::LinearMove(LinearMove(up_position_at_time(0.0))))
                                });
                            }
                        }
                        // If there is a transition for the minimum within the interval, execute it.
                        if min > 0.0 {
                            // Travel to the minimum point on the actual path
                            lines.push(Line {
                                modal_updates: get_original_modal_updates()?,
                                command: Some(CommandContent::LinearMove(LinearMove(position_at_time(min))))
                            });
                            // Travel up
                            lines.push(Line {
                                modal_updates: get_vertical_modal_updates(),
                                command: Some(CommandContent::LinearMove(LinearMove(up_position_at_time(min))))
                            });
                        }
                        // Travel across the compensated area
                        lines.push(Line {
                            modal_updates: get_original_modal_updates()?,
                            command: Some(CommandContent::LinearMove(LinearMove(up_position_at_time(max))))
                        });
                        // If there is a transition for the maximum within the interval, execute it.
                        if max < 1.0 {
                            // Travel down
                            lines.push(Line {
                                modal_updates: get_vertical_modal_updates(),
                                command: Some(CommandContent::LinearMove(LinearMove(position_at_time(max))))
                            });
                            // Travel the rest of the way
                            lines.push(Line {
                                modal_updates: get_original_modal_updates()?,
                                command: Some(CommandContent::LinearMove(LinearMove(position_at_time(1.0))))
                            });
                        }
                        self.is_compensated = max == 1.0;
                    },
                    None => {
                        if self.is_compensated {
                            lines.push(Line {
                                modal_updates: get_vertical_modal_updates(),
                                command: Some(CommandContent::LinearMove(LinearMove(position_at_time(0.0))))
                            });
                        }
                        lines.push(Line {
                            modal_updates: get_original_modal_updates()?,
                            command: Some(CommandContent::LinearMove(LinearMove(position_at_time(1.0))))
                        });
                        self.is_compensated = false;
                    },
                }
                Ok(
                    lines.into_iter()
                )
            },
            Some(CommandContent::HelicalMove(_)) => {
                Err(TagError::Unsupported)
            },
            Some(CommandContent::ProbeMove(_)) => Err(TagError::Unsupported),
            None => Ok(vec![line].into_iter()),
        }
    }
}

#[cfg(test)]
mod test {
    use itertools::Itertools;

    use crate::{config::MachineConfiguration, output::MachineFormatter};

    use super::*;
    fn is_close(lhs: Interval, rhs: Interval) -> bool {
        match (lhs.0, rhs.0) {
            (None, None) => true,
            (Some((lhs_min, lhs_max)), Some((rhs_min, rhs_max))) => (lhs_min - rhs_min).abs() < 0.001 && (lhs_max - rhs_max).abs() < 0.001,
            _ => false,
        }
    }

    /// A tag that has a point at (4.0, 6.0) for testing.
    fn simple_tag() -> Tag {
        Tag { position: (1.0, 2.0), minimum_height: 10.0, radius: 5.0 }
    }

    #[test]
    fn bad_progress_interval_simple_crossing() {
        // Hits point (4, 6), for instance
        let result = bad_progress_interval(
            PartialPosition(vec![Some(3.0), Some(6.0), Some(5.0)]),
            PartialPosition(vec![Some(5.0), Some(6.0), Some(5.0)]),
            &simple_tag()        
        ).unwrap();
        assert!(is_close(result, Interval(Some((0.0, 0.5)))))
    }
    #[test]
    fn bad_progress_interval_too_high() {
        // Hits point (4, 6), for instance
        let result = bad_progress_interval(
            PartialPosition(vec![Some(3.0), Some(6.0), Some(15.0)]),
            PartialPosition(vec![Some(5.0), Some(6.0), Some(15.0)]),
            &simple_tag()        
        ).unwrap();
        assert!(is_close(result, Interval(None)))
    }
    #[test]
    fn bad_progress_interval_too_high_slant() {
        // Hits point (4, 6), for instance
        let result = bad_progress_interval(
            PartialPosition(vec![Some(3.0), Some(6.0), Some(15.0)]),
            PartialPosition(vec![Some(5.0), Some(6.0), Some(6.0)]),
            &simple_tag()        
        ).unwrap();
        assert!(is_close(result, Interval(None)))
    }
    #[test]
    fn bad_progress_interval_slant_into() {
        // Hits point (4, 6), for instance
        let result = bad_progress_interval(
            PartialPosition(vec![Some(3.0), Some(6.0), Some(15.0)]),
            PartialPosition(vec![Some(5.0), Some(6.0), Some(-5.0)]),
            &simple_tag()        
        ).unwrap();
        assert!(is_close(result, Interval(Some((0.25, 0.5)))))
    }
    #[test]
    fn bad_progress_interval_slant_out() {
        // Hits point (4, 6), for instance
        let result = bad_progress_interval(
            PartialPosition(vec![Some(3.0), Some(6.0), Some(5.0)]),
            PartialPosition(vec![Some(5.0), Some(6.0), Some(25.0)]),
            &simple_tag()        
        ).unwrap();
        assert!(is_close(result, Interval(Some((0.0, 0.25)))))
    }
    #[test]
    fn bad_progress_interval_vertical_out() {
        // Hits point (4, 6), for instance
        let result = bad_progress_interval(
            PartialPosition(vec![Some(3.0), Some(6.0), Some(5.0)]),
            PartialPosition(vec![Some(3.0), Some(6.0), Some(25.0)]),
            &simple_tag()        
        ).unwrap();
        assert!(is_close(result, Interval(Some((0.0, 0.25)))))
    }
    #[test]
    fn bad_progress_interval_vertical_outside() {
        // Hits point (4, 6), for instance
        let result = bad_progress_interval(
            PartialPosition(vec![Some(5.0), Some(6.0), Some(5.0)]),
            PartialPosition(vec![Some(5.0), Some(6.0), Some(25.0)]),
            &simple_tag()        
        ).unwrap();
        assert!(is_close(result, Interval(None)))
    }
    #[test]
    fn apply_to_path() {
        // Cross over at (-2, 6) and (4, 6)
        let line = Line {
            modal_updates: ModalUpdates { feedrate: None, motion_mode: Some(MotionMode::Controlled), coordinate_mode: None, units: None, arc_plane: None, coordinate_system: None },
            command: Some(CommandContent::LinearMove(LinearMove(PartialPosition(vec![
                Some(7.0),
                Some(6.0),
                Some(4.0)
            ]))))
        };
        let mut start = MachineState::new(3);
        start.position = PartialPosition(vec![
            Some(-5.0),
            Some(6.0),
            Some(0.0)
        ]);
        let mut applier = TagApplier::new(simple_tag(), None);
        let result = applier.apply_to(&start, line).unwrap().collect_vec();
        let config = MachineConfiguration::standard_3_axis();
        let file = result.iter().map(|item| MachineFormatter(&config, item).to_string()).collect_vec().join("\n");
        assert_eq!(file, r"
G1 X-2.000 Y6.000 Z1.000
G1 X-2.000 Y6.000 Z10.000
G1 X4.000 Y6.000 Z10.000
G1 X4.000 Y6.000 Z3.000
G1 X7.000 Y6.000 Z4.000
".trim())
    }
}