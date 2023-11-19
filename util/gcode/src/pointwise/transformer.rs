use crate::{config::MachineConfiguration, gcode::{Line, CommandContent, LinearMove, ProbeMove, HelicalMove, ModalUpdates, MachineState}, parse::parse_line, output::MachineFormatter, coordinates::{Sign, ArcPlane, PartialPosition}};

pub struct CommandTransformer<A> {
    transformation: A,
    position: PartialPosition,
}
pub enum CommandError<E> {
    HelicalMoveEncountered,
    TransformError(E),
}
impl<E, A: FnMut(PartialPosition) -> Result<PartialPosition, E>> CommandTransformer<A> {
    pub fn new(transformation: A, position: PartialPosition) -> Self {
        CommandTransformer { transformation, position }
    }
    pub fn transform(&mut self, line: &Line) -> Result<Line, CommandError<E>> {
        let command = match &line.command {
            Some(CommandContent::LinearMove(LinearMove(target))) => {
                self.position.update_from(target);
                Some(CommandContent::LinearMove(LinearMove((self.transformation)(self.position.clone()).map_err(CommandError::TransformError)?)))
            },
            Some(CommandContent::ProbeMove(ProbeMove(mode, target))) => {
                self.position.update_from(target);
                Some(CommandContent::ProbeMove(ProbeMove(*mode, (self.transformation)(self.position.clone()).map_err(CommandError::TransformError)?)))
            },            Some(CommandContent::HelicalMove(_)) => return Err(CommandError::HelicalMoveEncountered),
            None => None,
        };
        Ok(Line {
            modal_updates: ModalUpdates {
                ..line.modal_updates
            },
            command,
        })
    }
}

pub fn transform_gcode_file<E, A: FnMut(PartialPosition) -> Result<PartialPosition, E>>(
    config: &MachineConfiguration,
    transform: A,
    input: &str,
) -> Result<String, usize> {
    let mut transformer = CommandTransformer::new(
        transform,
        PartialPosition::empty(config.axis_characters.len() as u8)
    );
    let mut result = input.lines().enumerate().map(|(index, line)|
        if line.trim_start().starts_with("(") || line.trim_start().starts_with("M") {
            Ok(format!("{}\n", line))
        } else {
            parse_line(config, line)
            .and_then(|line|
                transformer.transform(&line).ok()
            )
            .map(|line|
                format!("{}\n", MachineFormatter(config, &line))
            ).ok_or_else(
                || index
            )
        }
    ).collect::<Result<String, usize>>()?;
    // Trim ending whitespace to avoid repeated transformations adding lots of empty lines
    // at the end.
    loop {
        match result.pop() {
            Some(c) if c.is_whitespace() => (),
            Some(c) => {
                result.push(c);
                result.push('\n');
                return Ok(result)
            }
            None => return Ok(result)
        }
    }
}


