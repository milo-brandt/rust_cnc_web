use std::num::ParseFloatError;

use nom::{
    bytes::complete::{tag, take_while},
    character::complete::{alpha1, space0, space1},
    combinator::{fail, map_res},
    error::{FromExternalError, ParseError},
    IResult, Offset, Parser,
};

use crate::cnc::gcode::{CoordinateMode, CoordinateSystem, Plane, SpindleMode, Unit};

use super::{
    AxisValues, GCodeCommand, GCodeFormatSpecification, GCodeLine, GCodeModal, MoveMode,
    OffsetAxisValues, Orientation, ProbeDirection, ProbeRequirement,
};

enum PrimaryCommand {
    Move(MoveMode),
    ArcMove(Orientation),
    Dwell,
    SetWorkCoordinate,
    Probe(ProbeDirection, ProbeRequirement),
}

struct PartialGCodeLine {
    modals: Vec<GCodeModal>,
    primary_command: Option<PrimaryCommand>,
    axis_values: AxisValues,
    offset_axis_values: OffsetAxisValues,
    p_value: Option<f64>,
}
fn parse_f64<'a, Error: 'a + ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, f64, Error>
where
    Error: FromExternalError<&'a str, ParseFloatError>,
{
    map_res(
        take_while(|c: char| c.is_ascii_digit() || c == '.' || c == '-'),
        |substr: &str| substr.parse::<f64>(),
    )
    .parse(input)
}
macro_rules! extract_input {
    ( $name: ident,  $x: expr ) => {{
        let (input, value) = $x($name)?;
        $name = input;
        value
    }};
}
enum GCodePart<'a> {
    G(&'a str),
    M(&'a str),
    P(f64),
    F(f64),
    AxisWord(usize, f64),
    OffsetAxisWord(usize, f64),
}
fn parse_part<'a, 'b, Error: 'a + ParseError<&'a str>>(
    spec: &'b GCodeFormatSpecification,
) -> impl Fn(&'a str) -> IResult<&'a str, GCodePart<'a>, Error> + 'b
where
    Error: FromExternalError<&'a str, ParseFloatError>,
{
    // Assume we've already deal with whitespace
    |true_start| {
        let (mut input, head) = alpha1(true_start)?;
        match head {
            "G" => {
                let name =
                    extract_input!(input, take_while(|c: char| c.is_ascii_digit() || c == '-'));
                Ok((input, GCodePart::G(name)))
            }
            "M" => {
                let name =
                    extract_input!(input, take_while(|c: char| c.is_ascii_digit() || c == '-'));
                Ok((input, GCodePart::M(name)))
            }
            "P" => {
                let value = extract_input!(input, parse_f64);
                Ok((input, GCodePart::P(value)))
            }
            "F" => {
                let value = extract_input!(input, parse_f64);
                Ok((input, GCodePart::F(value)))
            }
            head if head.len() == 1 => match parse_f64::<Error>(input) {
                Ok((input, value)) => {
                    let head = head.bytes().next().unwrap();
                    for (index, axis_letter) in spec.axis_letters.iter().enumerate() {
                        if *axis_letter == head {
                            return Ok((input, GCodePart::AxisWord(index, value)));
                        }
                    }
                    for (index, axis_letter) in spec.offset_axis_letters.iter().enumerate() {
                        if *axis_letter == head {
                            return Ok((input, GCodePart::OffsetAxisWord(index, value)));
                        }
                    }
                    fail(true_start)
                }
                _ => fail(true_start),
            },
            _ => fail(true_start),
        }
    }
}

fn parse_gcode_line<'a, 'b, Error: 'a + ParseError<&'a str>>(
    spec: &'b GCodeFormatSpecification,
) -> impl Fn(&'a str) -> IResult<&'a str, GCodeLine, Error> + 'b
where
    Error: FromExternalError<&'a str, ParseFloatError>,
{
    |mut input| {
        let mut line = PartialGCodeLine {
            modals: vec![],
            primary_command: None,
            axis_values: AxisValues(vec![]),
            offset_axis_values: OffsetAxisValues(vec![]),
            p_value: None,
        };
        let set_primary = |line: &mut PartialGCodeLine, input: &'a str, primary_command| {
            if line.primary_command.is_some() {
                fail(input).map(|_: (_, ())| ())
            } else {
                line.primary_command = Some(primary_command);
                Ok(())
            }
        };
        loop {
            extract_input!(input, space0);
            if input.is_empty() {
                break;
            }
            let part = extract_input!(input, parse_part(spec));
            match part {
                GCodePart::G("0") => {
                    set_primary(&mut line, input, PrimaryCommand::Move(MoveMode::Rapid))?
                }
                GCodePart::G("1") => {
                    set_primary(&mut line, input, PrimaryCommand::Move(MoveMode::Controlled))?
                }
                GCodePart::G("2") => set_primary(
                    &mut line,
                    input,
                    PrimaryCommand::ArcMove(Orientation::Clockwise),
                )?,
                GCodePart::G("3") => set_primary(
                    &mut line,
                    input,
                    PrimaryCommand::ArcMove(Orientation::Counterclockwise),
                )?,
                GCodePart::G("4") => set_primary(&mut line, input, PrimaryCommand::Dwell)?,
                GCodePart::G("38.2") => set_primary(
                    &mut line,
                    input,
                    PrimaryCommand::Probe(ProbeDirection::Towards, ProbeRequirement::Require),
                )?,
                GCodePart::G("38.3") => set_primary(
                    &mut line,
                    input,
                    PrimaryCommand::Probe(ProbeDirection::Towards, ProbeRequirement::Optional),
                )?,
                GCodePart::G("38.4") => set_primary(
                    &mut line,
                    input,
                    PrimaryCommand::Probe(ProbeDirection::Away, ProbeRequirement::Require),
                )?,
                GCodePart::G("38.5") => set_primary(
                    &mut line,
                    input,
                    PrimaryCommand::Probe(ProbeDirection::Away, ProbeRequirement::Optional),
                )?,
                GCodePart::G("10") => {
                    extract_input!(input, space1);
                    extract_input!(input, tag("L20"));
                    set_primary(&mut line, input, PrimaryCommand::SetWorkCoordinate)?
                }
                GCodePart::G("17") => line.modals.push(GCodeModal::SetArcPlane(Plane::XY)),
                GCodePart::G("18") => line.modals.push(GCodeModal::SetArcPlane(Plane::ZX)),
                GCodePart::G("19") => line.modals.push(GCodeModal::SetArcPlane(Plane::YZ)),
                GCodePart::G("20") => line.modals.push(GCodeModal::SetUnits(Unit::Inch)),
                GCodePart::G("21") => line.modals.push(GCodeModal::SetUnits(Unit::Millimeter)),
                GCodePart::G("54") => line
                    .modals
                    .push(GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord0)),
                GCodePart::G("55") => line
                    .modals
                    .push(GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord1)),
                GCodePart::G("56") => line
                    .modals
                    .push(GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord2)),
                GCodePart::G("57") => line
                    .modals
                    .push(GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord3)),
                GCodePart::G("58") => line
                    .modals
                    .push(GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord4)),
                GCodePart::G("59") => line
                    .modals
                    .push(GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord5)),
                GCodePart::G("90") => line
                    .modals
                    .push(GCodeModal::SetCoordinateMode(CoordinateMode::Absolute)),
                GCodePart::G("91") => line
                    .modals
                    .push(GCodeModal::SetCoordinateMode(CoordinateMode::Incremental)),
                GCodePart::M("2") => line.modals.push(GCodeModal::EndProgram),
                GCodePart::M("3") => line
                    .modals
                    .push(GCodeModal::SetSpindle(SpindleMode::Clockwise)),
                GCodePart::M("5") => line.modals.push(GCodeModal::SetSpindle(SpindleMode::Off)),
                GCodePart::G(_) => return fail(input),
                GCodePart::M(_) => return fail(input),
                GCodePart::P(value) => {
                    if line.p_value.is_none() {
                        line.p_value = Some(value);
                    } else {
                        return fail(input);
                    }
                }
                GCodePart::F(value) => line.modals.push(GCodeModal::SetFeedrate(value)),
                GCodePart::AxisWord(index, value) => line.axis_values.0.push((index, value)),
                GCodePart::OffsetAxisWord(index, value) => {
                    line.offset_axis_values.0.push((index, value))
                }
            }
        }
        // Process the line
        let gcode_command = match line.primary_command {
            Some(PrimaryCommand::Move(mode)) => Some(GCodeCommand::Move {
                mode,
                position: line.axis_values,
            }),
            Some(PrimaryCommand::Dwell) => match line.p_value {
                Some(p_value) => Some(GCodeCommand::Dwell { duration: p_value }),
                None => return fail(input),
            },
            Some(PrimaryCommand::ArcMove(orientation)) => Some(GCodeCommand::ArcMove {
                orientation,
                position: line.axis_values,
                offsets: line.offset_axis_values,
                revolutions: line.p_value.map(|f| f as u64),
            }),
            Some(PrimaryCommand::Probe(mode, requirement)) => Some(GCodeCommand::Probe {
                position: line.axis_values,
                mode,
                requirement,
            }),
            Some(PrimaryCommand::SetWorkCoordinate) => {
                Some(GCodeCommand::SetWorkCoordinateTo(line.axis_values))
            }
            None => None,
        };
        Ok((
            input,
            GCodeLine {
                modals: line.modals,
                command: gcode_command,
            },
        ))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn default_settings() -> GCodeFormatSpecification {
        GCodeFormatSpecification {
            axis_letters: b"XYZA".to_vec(),
            offset_axis_letters: b"IJK".to_vec(),
            float_digits: 2,
        }
    }

    #[test]
    fn test_parse() {
        let input = "G10 L2 X5 Y123 Z23 G90 G21 F250";
        let result: Result<_, nom::Err<()>> = parse_gcode_line(&default_settings())(input);
        println!("{:?}", result);
    }
}
