use std::marker::PhantomData;

use itertools::Either;

use crate::{gcode::{MachineState, Line, CommandContent, LinearMove, HelicalMove, ProbeMove}, coordinates::PartialPosition, config::MachineConfiguration, parse::parse_line};

#[derive(Debug)]
pub enum LinesError {
    UnknownArcPlane,
    MismatchedArcRadii,
}
pub enum ArcMode {
    Inscribed,
    Circumscribed,
    Midpoint
}
pub struct LinesConfiguration {
    arc_mode: ArcMode,
    tolerance: f64,
    arc_radii_tolerance: f64,
}
impl LinesConfiguration {
    // Returns an iterator yielding some series of possible targets...
    pub fn lines(&self, pre_machine_state: &MachineState, line: &Line) -> Result<impl Iterator<Item = PartialPosition>, LinesError> {
        match &line.command {
            Some(CommandContent::LinearMove(LinearMove(target))) => {
                Ok(Some(pre_machine_state.position.clone().or(target)).into_iter())
            },
            Some(CommandContent::HelicalMove(HelicalMove { orientation, target, center })) => {
                Ok(Some(pre_machine_state.position.clone().or(target)).into_iter())
            },
            Some(CommandContent::ProbeMove(ProbeMove(_, target))) => {
                Ok(Some(pre_machine_state.position.clone().or(target)).into_iter())
            },
            None => Ok(None.into_iter()),
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
            arc_mode: ArcMode::Inscribed,
            tolerance: 0.001,
            arc_radii_tolerance: 0.001,
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
            arc_mode: ArcMode::Inscribed,
            tolerance: 0.001,
            arc_radii_tolerance: 0.001,
        };
        let result = gcode_file_to_lines(config, machine, &lines_configuration, input).unwrap();
        assert_eq!(result, vec![
            PartialPosition(vec![None, None, Some(10.0)]),
            PartialPosition(vec![Some(0.0), None, Some(10.0)]),
            PartialPosition(vec![Some(0.0), Some(5.0), Some(10.0)]),
        ]);
    }
}