#[allow(unused_imports)] // This is used? Clippy thinks it's not for some reason
use ndarray::array;
use {
    super::messages::*,
    ndarray::Array1,
    nom::{
        self,
        branch::alt,
        bytes::complete::{tag, take_until, take_while},
        combinator::{all_consuming, fail, flat_map, map_parser, map_res, success},
        error::{FromExternalError, ParseError},
        multi::separated_list0,
        sequence::{delimited, preceded, separated_pair, terminated, tuple},
        Compare, FindSubstring, IResult, InputIter, InputLength, InputTake, Parser,
    },
    std::num::{ParseFloatError, ParseIntError},
};

enum GrblStatusPart {
    CurrentFeed(f64),
    CurrentSpindle(f64),
    Planner(u64),
    RxBytes(u64),
    WorkCoordinateOffset(Array1<f64>),
    LineNumber(u64),
    Pins(String),
    FeedOverride(f64),
    RapidOverride(f64),
    SpindleOverride(f64),
    AccessoryState(String),
    Unknown(String),
}
fn apply_grbl_status(mut status: GrblStatus, part: GrblStatusPart) -> GrblStatus {
    match part {
        GrblStatusPart::CurrentFeed(feed) => status.current_feed = Some(feed),
        GrblStatusPart::CurrentSpindle(spindle) => status.current_spindle = Some(spindle),
        GrblStatusPart::Planner(planner) => status.planner = Some(planner),
        GrblStatusPart::RxBytes(rx_bytes) => status.rx_bytes = Some(rx_bytes),
        GrblStatusPart::WorkCoordinateOffset(wco) => status.work_coordinate_offset = Some(wco),
        GrblStatusPart::LineNumber(line_number) => status.line_number = Some(line_number),
        GrblStatusPart::Pins(pins) => status.pins = Some(pins),
        GrblStatusPart::FeedOverride(fo) => status.feed_override = Some(fo),
        GrblStatusPart::RapidOverride(ro) => status.rapid_override = Some(ro),
        GrblStatusPart::SpindleOverride(so) => status.spindle_override = Some(so),
        GrblStatusPart::AccessoryState(accessories) => status.accessory_state = Some(accessories),
        GrblStatusPart::Unknown(unknown) => status.unknown_terms.push(unknown),
    };
    status
}

pub fn take_until_or_all<T, Input, Error: ParseError<Input>>(
    tag: T,
) -> impl Fn(Input) -> IResult<Input, Input, Error>
where
    Input: InputTake + InputLength + FindSubstring<T>,
    T: InputLength + Clone,
{
    move |input| {
        let size = match input.find_substring(tag.clone()) {
            Some(offset) => offset,
            None => input.input_len(),
        };
        let (suffix, prefix) = input.take_split(size);
        Ok((suffix, prefix))
    }
}
pub fn take_until_or_nonempty_all<T, Input, Error: ParseError<Input>>(
    tag: T,
) -> impl Fn(Input) -> IResult<Input, Input, Error>
where
    Input: InputTake + InputLength + FindSubstring<T>,
    T: InputLength + Clone,
{
    move |input| {
        let size = match input.find_substring(tag.clone()) {
            Some(offset) => offset,
            None => {
                let length = input.input_len();
                if length == 0 {
                    return fail(input);
                }
                length
            }
        };
        let (suffix, prefix) = input.take_split(size);
        Ok((suffix, prefix))
    }
}
pub fn all<Input, Error>(input: Input) -> IResult<Input, Input, Error>
where
    Input: InputTake + InputLength,
{
    Ok(input.take_split(input.input_len()))
}
fn split_by<T, Input, Error: ParseError<Input>>(
    separator: T,
) -> impl FnMut(Input) -> IResult<Input, Vec<Input>, Error>
where
    Input: InputTake + InputLength + FindSubstring<T> + Clone + InputIter + Compare<T>,
    T: InputLength + Clone,
{
    separated_list0(
        tag(separator.clone()),
        take_until_or_nonempty_all(separator),
    )
}
fn map_parser_vec<I, O1, O2, E, F, G>(
    mut parser: F,
    applied_parser: G,
) -> impl FnMut(I) -> IResult<I, Vec<O2>, E>
where
    E: ParseError<I>,
    E: ParseError<O1>,
    F: Parser<I, Vec<O1>, E>,
    G: Parser<O1, O2, E>,
    O1: InputLength,
{
    let mut applied_parser = all_consuming(applied_parser);
    move |input| {
        let (remaining, parts) = parser.parse(input)?;
        let result = parts
            .into_iter()
            .map(|part| applied_parser(part).map(|(_remaining, result)| result))
            .collect::<Result<Vec<O2>, _>>()?;
        Ok((remaining, result))
    }
}
fn split_by_then<T, I, O, E, F>(separator: T, parser: F) -> impl FnMut(I) -> IResult<I, Vec<O>, E>
where
    I: InputTake + InputLength + FindSubstring<T> + Clone + InputIter + Compare<T>,
    T: InputLength + Clone,
    E: ParseError<I>,
    F: Parser<I, O, E>,
{
    map_parser_vec(split_by(separator), parser)
}
fn map_consuming_parser<I, O1, O2, E, F, G>(
    parser: F,
    applied_parser: G,
) -> impl FnMut(I) -> IResult<I, O2, E>
where
    E: ParseError<I>,
    E: ParseError<O1>,
    O1: InputLength,
    F: Parser<I, O1, E>,
    G: Parser<O1, O2, E>,
{
    map_parser(parser, all_consuming(applied_parser))
}

fn take_until_through<'a, Error: 'a + ParseError<&'a str>>(
    separator: &'a str,
) -> impl 'a + FnMut(&'a str) -> IResult<&'a str, &'a str, Error> {
    terminated(take_until(separator), tag(separator))
}
fn take_until_through_or_all<'a, Error: 'a + ParseError<&'a str>>(
    separator: &'a str,
) -> impl 'a + FnMut(&'a str) -> IResult<&'a str, &'a str, Error> {
    terminated(take_until_or_all(separator), tag(separator).or(success("")))
}
fn enclosed_by<'a, Error: 'a + ParseError<&'a str>>(
    open: &'a str,
    close: &'a str,
) -> impl 'a + FnMut(&'a str) -> IResult<&'a str, &'a str, Error> {
    delimited(tag(open), take_until(close), tag(close))
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
fn parse_i64<'a, Error: 'a + ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, i64, Error>
where
    Error: FromExternalError<&'a str, ParseIntError>,
{
    map_res(
        take_while(|c: char| c.is_ascii_digit() || c == '-'),
        |substr: &str| substr.parse::<i64>(),
    )
    .parse(input)
}
fn parse_u64<'a, Error: 'a + ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, u64, Error>
where
    Error: FromExternalError<&'a str, ParseIntError>,
{
    map_res(take_while(|c: char| c.is_ascii_digit()), |substr: &str| {
        substr.parse::<u64>()
    })
    .parse(input)
}

/*
struct BoxedParser<I, O, E>(Box<dyn Parser<I, O, E>>);

impl<I, O, E> BoxedParser<I, O, E> {
    fn new<F: Parser<I, O, E>>(inner: F) -> Self {
        BoxedParser(Box::new(inner))
    }
}

impl<I, O, E> Parser<I, O, E> for BoxedParser<I, O, E> {
    fn parse(&mut self, input: I) -> IResult<I, O, E> {
        self.0.parse(input)
    }
}
*/

fn parse_grbl_state<'a, Error: 'a + ParseError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, GrblState, Error>
where
    Error: FromExternalError<&'a str, ParseIntError>,
{
    all_consuming(flat_map(
        take_until_through_or_all(":"),
        |head| -> Box<dyn Parser<&'a str, GrblState, Error>> {
            match head {
                "Idle" => Box::new(success(GrblState::Idle)),
                "Run" => Box::new(success(GrblState::Run)),
                "Hold" => Box::new(parse_i64.map(GrblState::Hold)),
                "Jog" => Box::new(success(GrblState::Jog)),
                "Alarm" => Box::new(success(GrblState::Alarm)),
                "Door" => Box::new(parse_i64.map(GrblState::Door)),
                "Check" => Box::new(success(GrblState::Check)),
                "Home" => Box::new(success(GrblState::Home)),
                "Sleep" => Box::new(success(GrblState::Sleep)),
                _ => Box::new(fail),
            }
        },
    ))
    .parse(input)
}
fn parse_float_array<'a, Error: 'a + ParseError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, Array1<f64>, Error>
where
    Error: FromExternalError<&'a str, ParseFloatError>,
{
    all_consuming(split_by_then(",", parse_f64).map(|floats| floats.into_iter().collect()))
        .parse(input)
}
fn parse_grbl_status_part<'a, Error: 'a + ParseError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, Vec<GrblStatusPart>, Error>
where
    Error: FromExternalError<&'a str, ParseFloatError>,
    Error: FromExternalError<&'a str, ParseIntError>,
{
    flat_map(
        take_until_through(":"),
        |head| -> Box<dyn Parser<&'a str, Vec<GrblStatusPart>, Error>> {
            match head {
                "WCO" => Box::new(
                    parse_float_array.map(|wco| vec![GrblStatusPart::WorkCoordinateOffset(wco)]),
                ),
                "Bf" => Box::new(separated_pair(parse_u64, tag(","), parse_u64).map(
                    |(planner, rx_bytes)| {
                        vec![
                            GrblStatusPart::Planner(planner),
                            GrblStatusPart::RxBytes(rx_bytes),
                        ]
                    },
                )),
                "Ln" => Box::new(parse_u64.map(|ln| vec![GrblStatusPart::LineNumber(ln)])),
                "F" => Box::new(parse_f64.map(|feed| vec![GrblStatusPart::CurrentFeed(feed)])),
                "FS" => Box::new(separated_pair(parse_f64, tag(","), parse_f64).map(
                    |(planner, rx_bytes)| {
                        vec![
                            GrblStatusPart::CurrentFeed(planner),
                            GrblStatusPart::CurrentSpindle(rx_bytes),
                        ]
                    },
                )),
                "Pn" => {
                    Box::new(all.map(|pins: &str| vec![GrblStatusPart::Pins(pins.to_string())]))
                }
                "Ov" => Box::new(
                    tuple((parse_f64, tag(","), parse_f64, tag(","), parse_f64)).map(
                        |(feed, _, rapids, _, spindle)| {
                            vec![
                                GrblStatusPart::FeedOverride(feed * 0.01),
                                GrblStatusPart::RapidOverride(rapids * 0.01),
                                GrblStatusPart::SpindleOverride(spindle * 0.01),
                            ]
                        },
                    ),
                ),
                "A" => Box::new(all.map(|accessories: &str| {
                    vec![GrblStatusPart::AccessoryState(accessories.to_string())]
                })),
                _ => Box::new(fail),
            }
        },
    )
    .or(all.map(|input: &str| vec![GrblStatusPart::Unknown(input.to_string())]))
    .parse(input)
}
fn parse_grbl_position<'a, Error: 'a + ParseError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, GrblPosition, Error>
where
    Error: FromExternalError<&'a str, ParseFloatError>,
{
    all_consuming(alt((
        preceded(tag("MPos:"), parse_float_array.map(GrblPosition::Machine)),
        preceded(tag("WPos:"), parse_float_array.map(GrblPosition::Work)),
    )))
    .parse(input)
}
fn parse_grbl_status<'a, Error: 'a + ParseError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, GrblStatus, Error>
where
    Error: FromExternalError<&'a str, ParseFloatError>,
    Error: FromExternalError<&'a str, ParseIntError>,
{
    let result = enclosed_by("<", ">")
        .and_then(
            take_until_through("|").and_then(parse_grbl_state).and(
                take_until_through_or_all("|")
                    .and_then(parse_grbl_position)
                    .and(split_by_then("|", parse_grbl_status_part)),
            ),
        )
        .parse(input);
    let (rest, (state, (position, pieces))) = result?;
    let status = GrblStatus::new(state, position);
    let status = pieces.into_iter().flatten().fold(status, apply_grbl_status);
    Ok((rest, status))
}
fn parse_grbl_square_brackets<'a, Error: 'a + ParseError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, GrblMessage, Error>
where
    Error: FromExternalError<&'a str, ParseFloatError>,
    Error: FromExternalError<&'a str, ParseIntError>,
{
    enclosed_by("[", "]")
        .and_then(flat_map(
            take_until_through(":"),
            |head| -> Box<dyn Parser<&'a str, GrblMessage, Error>> {
                match head {
                    "PRB" => Box::new(
                        separated_pair(parse_float_array, tag(":"), parse_u64.map(|u| u != 0)).map(
                            |(position, success)| GrblMessage::ProbeEvent { success, position },
                        ),
                    ),
                    _ => Box::new(fail),
                }
            },
        ))
        .parse(input)
}
fn parse_grbl_ok<'a, Error: 'a + ParseError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, GrblMessage, Error> {
    tag("ok").map(|_| GrblMessage::GrblOk).parse(input)
}
fn parse_grbl_error<'a, Error: 'a + ParseError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, GrblMessage, Error>
where
    Error: FromExternalError<&'a str, ParseIntError>,
{
    preceded(tag("error:"), parse_u64)
        .map(GrblMessage::GrblError)
        .parse(input)
}
fn parse_grbl_alarm<'a, Error: 'a + ParseError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, GrblMessage, Error>
where
    Error: FromExternalError<&'a str, ParseIntError>,
{
    preceded(tag("ALARM:"), parse_u64)
        .map(GrblMessage::GrblError)
        .parse(input)
}
fn parse_grbl_greeting<'a, Error: 'a + ParseError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, GrblMessage, Error> {
    tag("Grbl")
        .and(all)
        .map(|_| GrblMessage::GrblGreeting)
        .parse(input)
}
fn parse_grbl_line_impl<'a, Error: 'a + ParseError<&'a str>>(
    message: &'a str,
) -> IResult<&'a str, GrblMessage, Error>
where
    Error: FromExternalError<&'a str, ParseFloatError>,
    Error: FromExternalError<&'a str, ParseIntError>,
{
    alt((
        parse_grbl_status.map(GrblMessage::StatusEvent),
        parse_grbl_square_brackets,
        parse_grbl_ok,
        parse_grbl_error,
        parse_grbl_alarm,
        parse_grbl_greeting,
        all.map(|msg: &str| GrblMessage::Unrecognized(msg.to_string())),
    ))
    .parse(message)
}
pub fn parse_grbl_line(message: &str) -> GrblMessage {
    parse_grbl_line_impl::<()>(message)
        .expect("parsing always succeeds!")
        .1
}

#[cfg(test)]
mod tests {
    use nom::error::VerboseError;

    use super::*;
    /*
    Small tests
    */
    #[test]
    fn test_split_by() {
        let input = "a|b|ced";
        let result: Result<_, nom::Err<()>> = split_by("|").parse(input);
        assert_eq!(result, Ok(("", vec!["a", "b", "ced"])));
    }
    #[test]
    fn test_split_by_empty() {
        let input = "";
        let result: Result<_, nom::Err<()>> = split_by("|").parse(input);
        assert_eq!(result, Ok(("", vec![])));
    }
    #[test]
    fn test_enclosed_by() {
        let input = "<robert>outside";
        let result: Result<_, nom::Err<()>> = enclosed_by("<", ">").parse(input);
        assert_eq!(result, Ok(("outside", "robert")));
    }
    #[test]
    fn test_take_until_through() {
        let input = "abc|def";
        let result: Result<_, nom::Err<()>> = take_until_through("|").parse(input);
        assert_eq!(result, Ok(("def", "abc")));
    }
    #[test]
    fn test_parse_float() {
        let input = "-123.75blahblah";
        let result: Result<_, nom::Err<()>> = parse_f64(input);
        assert_eq!(result, Ok(("blahblah", -123.75)));
    }
    #[test]
    fn test_split_by_then() {
        let input = "10.5,1,-5.875";
        let result: Result<_, nom::Err<()>> = split_by_then(",", parse_f64).parse(input);
        assert_eq!(result, Ok(("", vec![10.5, 1.0, -5.875])));
    }
    #[test]
    fn test_take_until_through_or_all() {
        let input = "abc|def";
        let result: Result<_, nom::Err<()>> = take_until_through_or_all("|").parse(input);
        assert_eq!(result, Ok(("def", "abc")));
    }
    #[test]
    fn test_take_until_through_or_all_with_all() {
        let input = "abc";
        let result: Result<_, nom::Err<()>> = take_until_through_or_all("|").parse(input);
        assert_eq!(result, Ok(("", "abc")));
    }
    /*
    Tests showing combinations of inputs
    */
    #[test]
    fn test_map_enclosed_by_split_by() {
        let input = "<a|b|c>";
        let result: Result<_, nom::Err<()>> =
            enclosed_by("<", ">").and_then(split_by("|")).parse(input);
        assert_eq!(result, Ok(("", vec!["a", "b", "c"])));
    }
    #[test]
    fn test_map_enclosed_by_take_first_split_by() {
        let input = "<IDLE|xy|z>";
        let result: Result<_, nom::Err<()>> = (enclosed_by("<", ">")
            .and_then(take_until_through("|").and(split_by("|"))))
        .parse(input);
        assert_eq!(result, Ok(("", ("IDLE", vec!["xy", "z"]))));
    }
    #[test]
    fn test_parse_grbl_state() {
        let input = "Idle";
        let result: Result<_, nom::Err<VerboseError<_>>> = parse_grbl_state(input);
        assert_eq!(result, Ok(("", GrblState::Idle)));
    }
    #[test]
    fn test_parse_grbl_status() {
        let input = "<Idle|MPos:0.00,1.00,3.00|Pn:XY|WCO:5.00,-5.25,17>";
        let result: Result<_, nom::Err<VerboseError<_>>> = parse_grbl_status(input);
        let mut status = GrblStatus::new(
            GrblState::Idle,
            GrblPosition::Machine(array![0.0, 1.0, 3.0]),
        );
        status.work_coordinate_offset = Some(array![5.00, -5.25, 17.0]);
        status.pins = Some("XY".to_string());
        assert_eq!(result, Ok(("", status)));
    }
    #[test]
    fn test_parse_grbl_status_2() {
        let input = "<Idle|MPos:0.00,1.00,3.00|Pn:XY|WCO:5.00,-5.25,17|FS:100,500|Bf:15,128|Ov:25,50,200|A:SM|Unknown>";
        let result: Result<_, nom::Err<VerboseError<_>>> = parse_grbl_status(input);
        let mut status = GrblStatus::new(
            GrblState::Idle,
            GrblPosition::Machine(array![0.0, 1.0, 3.0]),
        );
        status.work_coordinate_offset = Some(array![5.00, -5.25, 17.0]);
        status.pins = Some("XY".to_string());
        status.current_feed = Some(100.0);
        status.current_spindle = Some(500.0);
        status.planner = Some(15);
        status.rx_bytes = Some(128);
        status.feed_override = Some(0.25);
        status.rapid_override = Some(0.5);
        status.spindle_override = Some(2.0);
        status.accessory_state = Some("SM".to_string());
        status.unknown_terms.push("Unknown".to_string());
        assert_eq!(result, Ok(("", status)));
    }
    #[test]
    fn test_parse_grbl_status_3() {
        let input = "<Idle|MPos:0.00,1.00,3.00>";
        let result: Result<_, nom::Err<VerboseError<_>>> = parse_grbl_status(input);
        let status = GrblStatus::new(
            GrblState::Idle,
            GrblPosition::Machine(array![0.0, 1.0, 3.0]),
        );
        assert_eq!(result, Ok(("", status)));
    }
    #[test]
    fn test_parse_grbl_status_4() {
        let input = "<Idle|WPos:0.00,1.00,3.00>";
        let result: Result<_, nom::Err<VerboseError<_>>> = parse_grbl_status(input);
        let status = GrblStatus::new(GrblState::Idle, GrblPosition::Work(array![0.0, 1.0, 3.0]));
        assert_eq!(result, Ok(("", status)));
    }
}
