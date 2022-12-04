// For now: just parse paths that consist of linear components without self-intersection.
// No checking is done.

use std::{num::ParseFloatError, mem};

use nom::{error::{ParseError, FromExternalError, ErrorKind}, IResult, combinator::map_res, bytes::complete::{take_while, tag}, Parser, sequence::separated_pair, character::complete::space0, Finish, number::complete::float};

enum Mode {
    MoveAbsolute,
    MoveRelative,
    LineAbsolute,
    LineRelative,
    HorizontalAbsolute,
    HorizontalRelative,
    VerticalAbsolute,
    VerticalRelative,
    ClosePath,
}

fn parse_float_pair<'a, Error: 'a + ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, (f32, f32), Error>
where
    Error: FromExternalError<&'a str, ParseFloatError>,
{
    separated_pair(float, tag(","), float).parse(input)
}

fn parse_mode<'a, Error: 'a + ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Mode, Error> {
    if input.is_empty() {
        return Err(nom::Err::Error(Error::from_error_kind(input, ErrorKind::Eof)));
    }
    match input.bytes().next() {
        None => Err(nom::Err::Error(Error::from_error_kind(input, ErrorKind::Eof))),
        Some(b'M') => Ok((&input[1..], Mode::MoveAbsolute)),
        Some(b'm') => Ok((&input[1..], Mode::MoveRelative)),
        Some(b'L') => Ok((&input[1..], Mode::LineAbsolute)),
        Some(b'l') => Ok((&input[1..], Mode::LineRelative)),
        Some(b'H') => Ok((&input[1..], Mode::HorizontalAbsolute)),
        Some(b'h') => Ok((&input[1..], Mode::HorizontalRelative)),
        Some(b'V') => Ok((&input[1..], Mode::VerticalAbsolute)),
        Some(b'v') => Ok((&input[1..], Mode::VerticalRelative)),
        Some(b'Z') => Ok((&input[1..], Mode::ClosePath)),
        Some(b'z') => Ok((&input[1..], Mode::ClosePath)),
        _ => Err(nom::Err::Error(Error::from_error_kind(input, ErrorKind::Char))),
    }
}


// Easier to write the general loop imperatively.

macro_rules! extract_input {
    ( $name: ident,  $x: expr ) => {{
        let (input, value) = $x($name)?;
        $name = input;
        value
    }};
}
macro_rules! extract_input_maybe {
    ( $name: ident,  $x: expr ) => {{
        let result: Result<_, nom::Err<()>> = $x($name);
        match result {
            Ok((input, value)) => {
                $name = input;
                Some(value)
            }
            Err(_) => None
        }
    }};
}
macro_rules! extract_after_whitespace_input_maybe {
    ( $name: ident,  $x: expr ) => {{
        extract_input!($name, space0);
        extract_input_maybe!($name, $x)
    }};
}
#[derive(Debug)]
pub struct Loop {
    pub positions: Vec<(f64, f64)>,
    pub closed: bool
}
impl Loop {
    fn new() -> Self {
        Loop {
            positions: Vec::new(),
            closed: false,
        }
    }
}

struct IntermediateState {
    loops: Vec<Loop>,
    open_loop: Loop,
    last_position: (f32, f32)
}
impl IntermediateState {
    fn new() -> Self {
        IntermediateState { loops: Vec::new(), open_loop: Loop::new(), last_position: (0.0, 0.0) }
    }
    fn finish_loop(&mut self) {
        let old_loop = mem::replace(&mut self.open_loop, Loop::new());
        if !old_loop.positions.is_empty() {
            self.loops.push(old_loop);
        }
    }
    fn push_position(&mut self, (x, y): (f32, f32)) {
        self.open_loop.positions.push((x as f64, y as f64));
        self.last_position = (x, y);
    }
    fn push_relative(&mut self, offset: (f32, f32)) {
        self.push_position(tuple_add(self.last_position, offset));
    }
    fn close_loop(&mut self) {
        if self.open_loop.positions.is_empty() {
            return;
        }
        if self.open_loop.positions[0] == self.open_loop.positions[self.open_loop.positions.len() - 1] {
            self.open_loop.positions.pop(); // remove duplication
        }
        let (x, y) = self.open_loop.positions[0];
        self.last_position = (x as f32, y as f32);
        self.open_loop.closed = true;
    }
}

fn tuple_add(lhs: (f32, f32), rhs: (f32, f32)) -> (f32, f32) {
    (lhs.0 + rhs.0, lhs.1 + rhs.1)
}


fn parse_path_impl<'a, Error: 'a + ParseError<&'a str>>(mut input: &'a str) -> IResult<&'a str, Vec<Loop>, Error>
where
    Error: FromExternalError<&'a str, ParseFloatError>,
{
    let mut state = IntermediateState::new();
    loop {
        extract_input!(input, space0);
        if input.is_empty() {
            state.finish_loop();
            return Ok((input, state.loops));
        }
        match extract_input!(input, parse_mode) {
            Mode::MoveAbsolute => {
                state.finish_loop();
                while let Some((x, y)) = extract_after_whitespace_input_maybe!(input, parse_float_pair) {
                    state.push_position((x, y));
                }
            }
            Mode::MoveRelative => {
                state.finish_loop();
                while let Some((dx, dy)) = extract_after_whitespace_input_maybe!(input, parse_float_pair) {
                    state.push_relative((dx, dy));
                }
            }
            Mode::LineAbsolute => {
                while let Some((x, y)) = extract_after_whitespace_input_maybe!(input, parse_float_pair) {
                    state.push_position((x, y));
                }
            }
            Mode::LineRelative => {
                while let Some((dx, dy)) = extract_after_whitespace_input_maybe!(input, parse_float_pair) {
                    state.push_relative((dx, dy));
                }
            },
            Mode::HorizontalAbsolute => {
                while let Some(x) = extract_after_whitespace_input_maybe!(input, float) {
                    state.push_position((x, state.last_position.1));
                }
            },
            Mode::HorizontalRelative => {
                while let Some(dx) = extract_after_whitespace_input_maybe!(input, float) {
                    state.push_relative((dx, 0.0));
                }
            },
            Mode::VerticalAbsolute => {
                while let Some(y) = extract_after_whitespace_input_maybe!(input, float) {
                    state.push_position((state.last_position.0, y));
                }
            },
            Mode::VerticalRelative => {
                while let Some(dy) = extract_after_whitespace_input_maybe!(input, float) {
                    state.push_relative((0.0, dy));
                }
            }
            Mode::ClosePath => {
                state.close_loop();
                state.finish_loop();
            }
        }
    }
}


pub fn parse_path<'a>(
    input: &'a str,
) -> Result<Vec<Loop>, nom::error::Error<&'a str>> {
    parse_path_impl(input)
        .finish()
        .map(|(_i, o)| o)
}
