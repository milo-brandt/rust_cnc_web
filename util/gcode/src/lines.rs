use std::{marker::PhantomData, ops::Sub, f64::consts::TAU};

use itertools::{Either, Itertools};

use crate::{gcode::{MachineState, Line, CommandContent, LinearMove, HelicalMove, ProbeMove, Orientation}, coordinates::PartialPosition, config::MachineConfiguration, parse::parse_line};

#[derive(Debug)]
pub enum LinesError {
    UnknownArcPlane,
    UnknownArcPosition,
    MismatchedArcRadii,
}
pub struct LinesConfiguration {
    pub tolerance: f64,
    pub arc_radii_tolerance: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, derive_more::Add, derive_more::Sub, derive_more::Mul)]
struct ArcVector(f64, f64);
impl ArcVector {
    pub fn angle_to(self) -> f64 {
        f64::atan2(self.1, self.0)
    }
    pub fn magnitude(self) -> f64 {
        (self.0 * self.0 + self.1 * self.1).sqrt()
    }
}
/// Returns the least value of the form value + step * i for integer i where value + step * i >= minimum. Assumes step > 0.
fn first_offset_at_least(minimum: f64, step: f64, value: f64) -> f64 {
    let steps = ((minimum - value) / step).ceil();
    value + steps * step
}
/// Returns the greatest value of the form value + step * i for integer i where value + step * i <= minimum. Assumes step > 0.
fn first_offset_at_most(minimum: f64, step: f64, value: f64) -> f64 {
    let steps = ((value - minimum) / step).ceil();
    value - steps * step
}


impl LinesConfiguration {
    // Returns an iterator yielding some series of possible targets...
    pub fn lines(&self, pre_machine_state: &MachineState, line: &Line) -> Result<impl Iterator<Item = PartialPosition>, LinesError> {
        match &line.command {
            Some(CommandContent::LinearMove(LinearMove(target))) => {
                Ok(Either::Left(Some(pre_machine_state.position.clone().or(target)).into_iter()))
            },
            Some(CommandContent::HelicalMove(HelicalMove { orientation, target, center, rotations })) => {
                let arc_plane = pre_machine_state.arc_plane.ok_or(LinesError::UnknownArcPlane)?;
                // If any target is specified along an unknown axis, bail.
                if pre_machine_state.position.0.iter().zip(target.0.iter()).any(|(before, target)| before.is_none() && target.is_some()) {
                    return Err(LinesError::UnknownArcPosition);
                }
                let start_position = ArcVector(
                    pre_machine_state.position.0[arc_plane.0 as usize].ok_or(LinesError::UnknownArcPosition)?,
                    pre_machine_state.position.0[arc_plane.1 as usize].ok_or(LinesError::UnknownArcPosition)?,
                );
                let end_position = ArcVector(
                    target.0[arc_plane.0 as usize].unwrap_or(start_position.0),
                    target.0[arc_plane.1 as usize].unwrap_or(start_position.1),
                );
                let center = ArcVector(
                    start_position.0 + center.0[arc_plane.0 as usize].unwrap_or(0.0),
                    start_position.1 + center.0[arc_plane.1 as usize].unwrap_or(0.0),
                );
                let start_radius = (start_position - center).magnitude();
                let end_radius = (end_position - center).magnitude();
                if (start_radius - end_radius).abs() > self.arc_radii_tolerance {
                    return Err(LinesError::MismatchedArcRadii)
                }
                let start_angle = (start_position - center).angle_to();
                let end_angle = if start_position == end_position {
                    match orientation {
                        Orientation::Clockwise => start_angle - TAU * *rotations as f64,
                        Orientation::Counterclockwise => start_angle + TAU * *rotations as f64,
                    }
                } else {
                    let end_angle = (end_position - center).angle_to();
                    match orientation {
                        Orientation::Clockwise => first_offset_at_most(start_angle, TAU, end_angle) - (*rotations - 1) as f64 * TAU,
                        Orientation::Counterclockwise => first_offset_at_least(start_angle, TAU, end_angle) + (*rotations - 1) as f64 * TAU,
                    }
                };
                let max_angle_step = 2.0 * (1.0 - self.tolerance / start_radius).acos();
                let steps = ((end_angle - start_angle) / max_angle_step).abs().max(1.0).ceil() as u64;
                let final_position = pre_machine_state.position.clone().or(target);
                let before = pre_machine_state.position.clone();
                let target = target.clone();
                let angles = (1..steps).map(move |step| {
                    let progress = step as f64 / steps as f64;
                    let angle = start_angle * (1.0 - progress) + end_angle * progress;
                    let radius = start_radius * (1.0 - progress) + end_radius * progress;
                    let mut interpolated_position = PartialPosition(
                        before.0.iter().zip(target.0.iter()).map(|(before, target)| {
                            match before {
                                None => None,
                                Some(before) => match target {
                                    None => Some(*before),
                                    Some(after) => Some(before * (1.0 - progress) + after * progress),
                                }
                            }
                        }).collect()
                    );
                    interpolated_position.0[arc_plane.0 as usize] = Some(f64::cos(angle) * radius + center.0);
                    interpolated_position.0[arc_plane.1 as usize] = Some(f64::sin(angle) * radius + center.1);
                    interpolated_position
                }).chain(Some(final_position).into_iter());
                Ok(Either::Right(angles.into_iter()))
            },
            Some(CommandContent::ProbeMove(ProbeMove(_, target))) => {
                Ok(Either::Left(Some(pre_machine_state.position.clone().or(target)).into_iter()))
            },
            None => Ok(Either::Left(None.into_iter())),
        }
    }
}

#[derive(Debug)]
pub struct EmptyIterator<T>(PhantomData<T>);
impl<T> Iterator for EmptyIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}
impl<T> Default for EmptyIterator<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}
/*
eitherable! { Outcome(Left, Right, Middle) }


eitherable_for! {
    [
        "std::Future": {
            "poll": Pin<&mut Self> ... -- can return Self or not reference it.
        }
    ]
}

*/

pub fn gcode_file_to_lines(
    config: &MachineConfiguration,
    mut state: MachineState,
    lines_configuration: &LinesConfiguration,
    input: &str,
) -> Result<Vec<PartialPosition>, (usize, Option<LinesError>)> {
    input.lines().enumerate().flat_map(|(index, line)| {
        if line.trim_start().starts_with("(") || line.trim_start().starts_with("M") {
            Either::Left(Either::Left(EmptyIterator::default()))
        } else {
            let line = match parse_line(config, line) {
                Some(line) => line,
                None => return Either::Left(Either::Right(Some(Err((index, None))).into_iter())),
            };
            let points = match lines_configuration.lines(&state, &line) {
                Ok(points) => points,
                Err(e) => return Either::Left(Either::Right(Some(Err((index, Some(e)))).into_iter())),
            };
            state.update_by(&line);
            Either::Right(points.map(Ok))
        }
    }).collect()
}

#[cfg(test)]
pub mod test {
    use itertools::Itertools;

    use crate::{config::MachineConfiguration, parse::parse_line};

    use super::*;

    #[test]
    pub fn test_lines() {
        let config = &MachineConfiguration::standard_4_axis();
        let line = "G1 Z10";
        let line = parse_line(config, line).unwrap();
        let mut machine = MachineState::new(4);
        let lines_configuration = LinesConfiguration {
            tolerance: 0.01,
            arc_radii_tolerance: 0.01,
        };
        machine.position.0[1] = Some(5.0);
        let output = lines_configuration.lines(&machine, &line).unwrap().collect_vec();
        assert_eq!(output, vec![
            PartialPosition(vec![None, Some(5.0), Some(10.0), None])
        ]);
    }
    #[test]
    pub fn test_file_lines() {
        let config = &MachineConfiguration::standard_3_axis();
        let input = r"
            G1 Z10
            G1 X0
            G1 Y5
        ";
        let machine = MachineState::new(3);
        let lines_configuration = LinesConfiguration {
            tolerance: 0.01,
            arc_radii_tolerance: 0.01,
        };
        let result = gcode_file_to_lines(config, machine, &lines_configuration, input).unwrap();
        assert_eq!(result, vec![
            PartialPosition(vec![None, None, Some(10.0)]),
            PartialPosition(vec![Some(0.0), None, Some(10.0)]),
            PartialPosition(vec![Some(0.0), Some(5.0), Some(10.0)]),
        ]);
    }
    fn are_close(expected: &Vec<PartialPosition>, result: &Vec<PartialPosition>) -> bool {
        result.len() == expected.len() && result.iter().zip(expected.iter()).all(
            |(result, expected)| expected.0.len() == result.0.len() && expected.0.iter().zip(result.0.iter()).all(
                |(result, expected)| match (result, expected) {
                    (None, None) => true,
                    (Some(x), Some(y)) => (x - y).abs() < 0.001,
                    _ => false,
                })
        )
    }

    #[test]
    pub fn test_arc() {
        let config = &MachineConfiguration::standard_3_axis();
        let input = r"
            G17
            G0 X10 Y0 Z10
            G2 I-10 X-10 Z0
        ";
        let machine = MachineState::new(3);
        let lines_configuration = LinesConfiguration {
            tolerance: 9.0,
            arc_radii_tolerance: 0.01,
        };
        let result = gcode_file_to_lines(config, machine, &lines_configuration, input).unwrap();
        let expected = vec![
            PartialPosition(vec![Some(10.0), Some(0.0), Some(10.0)]),
            PartialPosition(vec![Some(0.0), Some(-10.0), Some(5.0)]),
            PartialPosition(vec![Some(-10.0), Some(0.0), Some(0.0)]),
        ];
        assert!(are_close(&expected, &result));
    }
    #[test]
    pub fn test_arc_ccw() {
        let config = &MachineConfiguration::standard_3_axis();
        let input = r"
            G17
            G0 X10 Y0 Z10
            G3 I-10 X-10 Z0
        ";
        let machine = MachineState::new(3);
        let lines_configuration = LinesConfiguration {
            tolerance: 9.0,
            arc_radii_tolerance: 0.01,
        };
        let result = gcode_file_to_lines(config, machine, &lines_configuration, input).unwrap();
        let expected = vec![
            PartialPosition(vec![Some(10.0), Some(0.0), Some(10.0)]),
            PartialPosition(vec![Some(0.0), Some(10.0), Some(5.0)]),
            PartialPosition(vec![Some(-10.0), Some(0.0), Some(0.0)]),
        ];
        assert!(are_close(&expected, &result));
    }

    #[test]
    pub fn test_arc_turns() {
        let config = &MachineConfiguration::standard_3_axis();
        let input = r"
            G17
            G0 X10 Y0 Z10
            G2 I-10 X-10 Z0 P2
        ";
        let machine = MachineState::new(3);
        let lines_configuration = LinesConfiguration {
            tolerance: 3.2,
            arc_radii_tolerance: 0.01,
        };
        let result = gcode_file_to_lines(config, machine, &lines_configuration, input).unwrap();
        let expected = vec![
            PartialPosition(vec![Some(10.0), Some(0.0), Some(10.0)]),
            PartialPosition(vec![Some(0.0), Some(-10.0), Some(25.0/3.0)]),
            PartialPosition(vec![Some(-10.0), Some(0.0), Some(20.0/3.0)]),
            PartialPosition(vec![Some(0.0), Some(10.0), Some(5.0)]),
            PartialPosition(vec![Some(10.0), Some(0.0), Some(10.0/3.0)]),
            PartialPosition(vec![Some(0.0), Some(-10.0), Some(5.0/3.0)]),
            PartialPosition(vec![Some(-10.0), Some(0.0), Some(0.0)]),
        ];
        assert!(are_close(&expected, &result));
    }

}