use {
    super::{
        AxisValues, GCodeCommand, GCodeFormatSpecification, GCodeLine, GCodeModal, MoveMode,
        OffsetAxisValues, Orientation, ProbeDirection, ProbeRequirement,
    },
    crate::cnc::gcode::{CoordinateMode, CoordinateSystem, ArcPlane, SpindleMode, Unit},
    itertools::Itertools,
    nom::{
        bytes::complete::{tag, take_while},
        character::complete::{alpha1, space0, space1},
        combinator::{fail, map_res},
        error::{FromExternalError, ParseError},
        Finish, IResult, Parser,
    },
    std::{collections::HashMap, num::ParseFloatError},
};

#[derive(Debug)]
pub struct GCodeParseError<'a> {
    pub remaining: &'a str,
    pub description: String,
}
#[derive(Debug)]
pub struct GCodeParseErrorOwned {
    pub remaining: String,
    pub description: String,
}
impl<'a> GCodeParseError<'a> {
    pub fn into_owned(self) -> GCodeParseErrorOwned {
        GCodeParseErrorOwned {
            remaining: self.remaining.to_string(),
            description: self.description,
        }
    }
}
impl<'a> ParseError<&'a str> for GCodeParseError<'a> {
    fn from_error_kind(input: &'a str, _kind: nom::error::ErrorKind) -> Self {
        GCodeParseError {
            remaining: input,
            description: "unknown".to_string(),
        }
    }

    fn append(_: &'a str, _: nom::error::ErrorKind, other: Self) -> Self {
        other
    }
}
impl<'a> FromExternalError<&'a str, ParseFloatError> for GCodeParseError<'a> {
    fn from_external_error(
        input: &'a str,
        _kind: nom::error::ErrorKind,
        _e: ParseFloatError,
    ) -> Self {
        GCodeParseError {
            remaining: input,
            description: "unknown".to_string(),
        }
    }
}
fn map_error<I, O, E1, E2, P: Parser<I, O, E1>, F: FnMut(E1) -> E2>(
    mut parser: P,
    mut f: F,
) -> impl FnMut(I) -> IResult<I, O, E2> {
    move |input| match parser.parse(input) {
        Ok(v) => Ok(v),
        Err(nom::Err::Error(e)) => Err(nom::Err::Error(f(e))),
        Err(nom::Err::Failure(e)) => Err(nom::Err::Failure(f(e))),
        Err(nom::Err::Incomplete(ct)) => Err(nom::Err::Incomplete(ct)),
    }
}
fn map_error_description<'a, O, P: Parser<&'a str, O, GCodeParseError<'a>>, D: ToString>(
    parser: P,
    description: D,
) -> impl FnMut(&'a str) -> IResult<&'a str, O, GCodeParseError<'a>> {
    map_error(parser, move |e| GCodeParseError {
        remaining: e.remaining,
        description: description.to_string(),
    })
}
fn make_error<O, D: ToString>(input: &str, description: D) -> IResult<&str, O, GCodeParseError> {
    Err(nom::Err::Error(GCodeParseError {
        remaining: input,
        description: description.to_string(),
    }))
}

enum PrimaryCommand {
    Move(MoveMode),
    ArcMove(Orientation),
    Dwell,
    SetWorkCoordinate,
    Probe(ProbeDirection, ProbeRequirement),
}

struct PartialGCodeLine<'a> {
    modals: Vec<GCodeModal>,
    primary_command: Option<PrimaryCommand>,
    axis_values: AxisValues,
    offset_axis_values: OffsetAxisValues,
    other_values: HashMap<u8, (f64, &'a str)>, //TODO: It's really silly to separate the logic here; would make more sense to collect all, then pop off as needed.
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
        let (new_input, value) = $x($name)?;
        $name = new_input;
        value
    }};
}
enum GCodePart<'a> {
    G(&'a str),
    M(&'a str),
    F(f64),
    S(f64),
    AxisWord(usize, f64),
    OffsetAxisWord(usize, f64),
    Other(u8, f64),
}
fn parse_part<'a, 'b>(
    spec: &'b GCodeFormatSpecification,
) -> impl Fn(&'a str) -> IResult<&'a str, GCodePart<'a>, GCodeParseError<'a>> + 'b {
    // Assume we've already deal with whitespace
    |true_start| {
        let (mut input, head) = alpha1(true_start)?;
        match head {
            "G" => {
                let name = extract_input!(
                    input,
                    map_error_description(
                        take_while(|c: char| c.is_ascii_digit() || c == '.'),
                        "expected number after G"
                    )
                );
                Ok((input, GCodePart::G(name)))
            }
            "M" => {
                let name = extract_input!(
                    input,
                    map_error_description(
                        take_while(|c: char| c.is_ascii_digit()),
                        "expected number after M"
                    )
                );
                Ok((input, GCodePart::M(name)))
            }
            "F" => {
                let value = extract_input!(
                    input,
                    map_error_description(parse_f64, "expected number after F")
                );
                Ok((input, GCodePart::F(value)))
            }
            "S" => {
                let old_input = input;
                let value = extract_input!(
                    input,
                    map_error_description(parse_f64, "expected number after S")
                );
                if value < 0.0 {
                    return make_error(old_input, "spindle speed must be non-negative");
                }
                Ok((input, GCodePart::S(value)))
            }
            head if head.len() == 1 => match parse_f64::<GCodeParseError<'a>>(input) {
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
                    Ok((input, GCodePart::Other(head, value)))
                }
                _ => make_error(true_start, "axis letter must be followed by number"),
            },
            _ => make_error(true_start, "axis must be one letter"),
        }
    }
}
fn same_modal_group(lhs: &GCodeModal, rhs: &GCodeModal) -> bool {
    matches!(
        (lhs, rhs),
        (GCodeModal::SetFeedrate(_), GCodeModal::SetFeedrate(_))
            | (GCodeModal::SetUnits(_), GCodeModal::SetUnits(_))
            | (GCodeModal::SetArcPlane(_), GCodeModal::SetArcPlane(_))
            | (
                GCodeModal::SetCoordinateSystem(_),
                GCodeModal::SetCoordinateSystem(_)
            )
            | (
                GCodeModal::SetCoordinateMode(_),
                GCodeModal::SetCoordinateMode(_)
            )
            | (GCodeModal::SetSpindle(_), GCodeModal::SetSpindle(_))
            | (
                GCodeModal::SetSpindleSpeed(_),
                GCodeModal::SetSpindleSpeed(_)
            )
            | (GCodeModal::EndProgram, GCodeModal::EndProgram)
    )
}
macro_rules! append_modal {
    ( $position: ident, $modals: expr,  $x: expr ) => {{
        let new_modal = $x;
        for modal in $modals {
            if same_modal_group(&modal, &new_modal) {
                return make_error($position, "two modals from same group");
            }
        }
        $modals.push(new_modal);
    }};
}
macro_rules! append_axis_word {
    ( $position: ident, $axis_vec: expr, $index: ident, $value: ident ) => {{
        for (index, _) in $axis_vec {
            if *index == $index {
                return make_error($position, "repeated axis letter");
            }
        }
        $axis_vec.push(($index, $value));
    }};
}
fn parse_gcode_line_impl<'a, 'b>(
    spec: &'b GCodeFormatSpecification,
) -> impl Fn(&'a str) -> IResult<&'a str, GCodeLine, GCodeParseError<'a>> + 'b {
    |mut input| {
        let full_input = input;
        let mut line = PartialGCodeLine {
            modals: vec![],
            primary_command: None,
            axis_values: AxisValues(vec![]),
            offset_axis_values: OffsetAxisValues(vec![]),
            other_values: Default::default(),
        };
        let set_primary = |line: &mut PartialGCodeLine, input: &'a str, primary_command| {
            if line.primary_command.is_some() {
                fail(input).map(|_: (_, ())| ())
            } else {
                line.primary_command = Some(primary_command);
                Ok(())
            }
        };
        let mut machine_coordinates = false;
        loop {
            extract_input!(input, space0);
            if input.is_empty() {
                break;
            }
            let prior_input = input;
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
                    extract_input!(
                        input,
                        map_error_description(space1, "expected space then L20 after G10")
                    );
                    extract_input!(
                        input,
                        map_error_description(tag("L20"), "expected L20 after G10")
                    );
                    set_primary(&mut line, input, PrimaryCommand::SetWorkCoordinate)?
                }
                GCodePart::G("17") => append_modal!(
                    prior_input,
                    &mut line.modals,
                    GCodeModal::SetArcPlane(ArcPlane::XY)
                ),
                GCodePart::G("18") => append_modal!(
                    prior_input,
                    &mut line.modals,
                    GCodeModal::SetArcPlane(ArcPlane::ZX)
                ),
                GCodePart::G("19") => append_modal!(
                    prior_input,
                    &mut line.modals,
                    GCodeModal::SetArcPlane(ArcPlane::YZ)
                ),
                GCodePart::G("20") => append_modal!(
                    prior_input,
                    &mut line.modals,
                    GCodeModal::SetUnits(Unit::Inch)
                ),
                GCodePart::G("21") => append_modal!(
                    prior_input,
                    &mut line.modals,
                    GCodeModal::SetUnits(Unit::Millimeter)
                ),
                GCodePart::G("53") => {
                    if machine_coordinates {
                        return make_error(prior_input, "duplicate G53");
                    } else {
                        machine_coordinates = true;
                    }
                }
                GCodePart::G("54") => append_modal!(
                    prior_input,
                    &mut line.modals,
                    GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord0)
                ),
                GCodePart::G("55") => append_modal!(
                    prior_input,
                    &mut line.modals,
                    GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord1)
                ),
                GCodePart::G("56") => append_modal!(
                    prior_input,
                    &mut line.modals,
                    GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord2)
                ),
                GCodePart::G("57") => append_modal!(
                    prior_input,
                    &mut line.modals,
                    GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord3)
                ),
                GCodePart::G("58") => append_modal!(
                    prior_input,
                    &mut line.modals,
                    GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord4)
                ),
                GCodePart::G("59") => append_modal!(
                    prior_input,
                    &mut line.modals,
                    GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord5)
                ),
                GCodePart::G("90") => append_modal!(
                    prior_input,
                    &mut line.modals,
                    GCodeModal::SetCoordinateMode(CoordinateMode::Absolute)
                ),
                GCodePart::G("91") => append_modal!(
                    prior_input,
                    &mut line.modals,
                    GCodeModal::SetCoordinateMode(CoordinateMode::Incremental)
                ),
                GCodePart::M("2") => {
                    append_modal!(prior_input, &mut line.modals, GCodeModal::EndProgram)
                }
                GCodePart::M("3") => append_modal!(
                    prior_input,
                    &mut line.modals,
                    GCodeModal::SetSpindle(SpindleMode::Clockwise)
                ),
                GCodePart::M("5") => append_modal!(
                    prior_input,
                    &mut line.modals,
                    GCodeModal::SetSpindle(SpindleMode::Off)
                ),
                GCodePart::F(value) => append_modal!(
                    prior_input,
                    &mut line.modals,
                    GCodeModal::SetFeedrate(value)
                ),
                GCodePart::S(value) => append_modal!(
                    prior_input,
                    &mut line.modals,
                    GCodeModal::SetSpindleSpeed(value)
                ),
                GCodePart::G(_) => return make_error(prior_input, "unrecognized G code"),
                GCodePart::M(_) => return make_error(prior_input, "unrecognized M code"),
                GCodePart::Other(head, value) => {
                    if line.other_values.insert(head, (value, input)).is_some() {
                        return make_error(prior_input, "repeated axis letter");
                    }
                }
                GCodePart::AxisWord(index, value) => {
                    append_axis_word!(prior_input, &mut line.axis_values.0, index, value)
                }
                GCodePart::OffsetAxisWord(index, value) => {
                    append_axis_word!(prior_input, &mut line.offset_axis_values.0, index, value)
                }
            }
        }
        // Process the line
        let gcode_command = match line.primary_command {
            Some(PrimaryCommand::Move(mode)) => {
                if !line.offset_axis_values.0.is_empty() {
                    return make_error(full_input, "move contains offset axis words");
                }
                Some(GCodeCommand::Move {
                    mode,
                    position: line.axis_values,
                    machine_coordinates,
                })
            }
            Some(_) if machine_coordinates => {
                return make_error(full_input, "G53 with non-move command");
            }
            Some(PrimaryCommand::Dwell) => {
                if !line.axis_values.0.is_empty() {
                    return make_error(full_input, "dwell contains axis words");
                }
                if !line.offset_axis_values.0.is_empty() {
                    return make_error(full_input, "dwell contains offset axis words");
                }
                match line.other_values.remove_entry(&b'P') {
                    Some((_, (p_value, _))) => Some(GCodeCommand::Dwell { duration: p_value }),
                    None => return make_error(full_input, "dwell without P value"),
                }
            }
            Some(PrimaryCommand::ArcMove(orientation)) => {
                if line.axis_values.0.is_empty() {
                    return make_error(full_input, "arc move without axis words");
                }
                if line.offset_axis_values.0.is_empty() {
                    return make_error(full_input, "arc move without offset axis words");
                }
                Some(GCodeCommand::ArcMove {
                    orientation,
                    position: line.axis_values,
                    offsets: line.offset_axis_values,
                    revolutions: line
                        .other_values
                        .remove_entry(&b'P')
                        .map(|(_, (value, _))| value as u64),
                })
            }
            Some(PrimaryCommand::Probe(mode, requirement)) => {
                if line.axis_values.0.is_empty() {
                    return make_error(full_input, "probe without axis words");
                }
                if !line.offset_axis_values.0.is_empty() {
                    return make_error(full_input, "probe contains offset axis words");
                }
                Some(GCodeCommand::Probe {
                    position: line.axis_values,
                    mode,
                    requirement,
                })
            }
            Some(PrimaryCommand::SetWorkCoordinate) => {
                if line.axis_values.0.is_empty() {
                    return make_error(full_input, "set coordinates without axis words");
                }
                if !line.offset_axis_values.0.is_empty() {
                    return make_error(full_input, "set coordinates contains offset axis words");
                }
                Some(GCodeCommand::SetWorkCoordinateTo(line.axis_values))
            }
            None => {
                if !line.offset_axis_values.0.is_empty() {
                    return make_error(full_input, "anonymous line contains offset axis words");
                }
                if line.axis_values.0.is_empty() {
                    if machine_coordinates {
                        return make_error(full_input, "G53 without axis words");
                    } else {
                        None
                    }
                } else {
                    Some(GCodeCommand::Move {
                        mode: MoveMode::Unspecified,
                        position: line.axis_values,
                        machine_coordinates,
                    })
                }
            }
        };
        if !line.other_values.is_empty() {
            return make_error(
                full_input,
                format!(
                    "unrecognized letters: {}",
                    line.other_values.into_keys().format(", ")
                ),
            );
        }
        Ok((
            input,
            GCodeLine {
                modals: line.modals,
                command: gcode_command,
            },
        ))
    }
}
#[derive(Debug)]
pub enum GeneralizedLine<'a> {
    Line(GCodeLine),
    Comment(&'a str),
    Empty,
}
fn parse_generalized_line_impl<'a, 'b>(
    spec: &'b GCodeFormatSpecification,
) -> impl Fn(&'a str) -> IResult<&'a str, GeneralizedLine<'a>, GCodeParseError<'a>> + 'b {
    move |mut input| {
        extract_input!(input, space0);
        if input.is_empty() {
            Ok((input, GeneralizedLine::Empty))
        } else if input.starts_with('(') {
            Ok((&input[input.len()..], GeneralizedLine::Comment(input)))
        } else {
            parse_gcode_line_impl(spec)
                .map(GeneralizedLine::Line)
                .parse(input)
        }
    }
}
pub fn parse_gcode_line<'a>(
    spec: &GCodeFormatSpecification,
    line: &'a str,
) -> Result<GCodeLine, GCodeParseError<'a>> {
    parse_gcode_line_impl(spec)
        .parse(line)
        .finish()
        .map(|(_i, o)| o)
}
pub fn parse_generalized_line<'a>(
    spec: &GCodeFormatSpecification,
    line: &'a str,
) -> Result<GeneralizedLine<'a>, GCodeParseError<'a>> {
    parse_generalized_line_impl(spec)
        .parse(line)
        .finish()
        .map(|(_i, o)| o)
}
#[derive(Debug)]
pub enum GeneralizedLineOwned {
    Line(GCodeLine),
    Comment(String),
    Empty,
}
impl<'a> GeneralizedLine<'a> {
    pub fn into_owned(self) -> GeneralizedLineOwned {
        match self {
            GeneralizedLine::Line(line) => GeneralizedLineOwned::Line(line),
            GeneralizedLine::Comment(comment) => GeneralizedLineOwned::Comment(comment.to_string()),
            GeneralizedLine::Empty => GeneralizedLineOwned::Empty,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn default_settings() -> GCodeFormatSpecification {
        GCodeFormatSpecification {
            axis_letters: b"XYZA".to_vec(),
            offset_axis_letters: b"IJK".to_vec(),
            float_digits: 3,
        }
    }

    #[test]
    fn test_parse() {
        let input = "G10 L20 X5 Y123 Z23 G90 G21 F250";
        let result: Result<_, _> = parse_gcode_line_impl(&default_settings())(input);
        println!("{:?}", result);
    }

    #[test]
    fn test_good_examples() {
        for input in &[
            include_str!("test_data/disk_job.nc"),
            include_str!("test_data/front_face.nc"),
            include_str!("test_data/go_home.nc"),
        ] {
            for line in input.lines() {
                let result = parse_generalized_line(&default_settings(), line);
                println!("Line: {:?}", line);
                println!("\tResult: {:?}", result);
                if let Ok(GeneralizedLine::Line(gcode)) = &result {
                    println!("\tReparsed: {}", default_settings().format_line(gcode));
                }
                assert!(result.is_ok());
            }
        }
    }
}
