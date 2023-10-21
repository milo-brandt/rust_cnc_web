use crate::{config::MachineConfiguration, gcode::{Line, CommandContent, LinearMove, ProbeMove, HelicalMove, ModalUpdates}, parse::parse_line, output::MachineFormatter, coordinates::{Sign, ArcPlane}};

use super::{SimpleTransform, transform::Transform};

pub struct CommandTransformer<'a> {
    orientation_sign: Option<Sign>,
    planes: Vec<ArcPlane>,
    transformation: &'a SimpleTransform,
}
pub enum CommandTransformError {
    UnknownOrientationSign,
    InvalidArcPlane(u8, u8),
}
impl<'a> CommandTransformer<'a> {
    pub fn new(transformation: &'a SimpleTransform, planes: Vec<ArcPlane>) -> Self {
        CommandTransformer {
            // If this is a translation, we don't need to re-orient anything, even if we don't know the arc plane.
            orientation_sign: if transformation.is_translation() { Some(Sign::Positive) } else { None },
            planes,
            transformation
        }
    }
    pub fn transform(&mut self, line: &Line) -> Result<Line, CommandTransformError> {
        let arc_plane = if let Some(arc_plane) = line.modal_updates.arc_plane {
            let first_index = self.transformation.permutation[arc_plane.0 as usize];
            let second_index = self.transformation.permutation[arc_plane.1 as usize];
            let desired_plane = ArcPlane(first_index.1, second_index.1);
            let (new_plane, new_sign) = self.planes
                .iter()
                .find_map(|plane| ArcPlane::compare(plane, &desired_plane).map(|sign| (plane, sign)))
                .ok_or_else(|| CommandTransformError::InvalidArcPlane(first_index.1, second_index.1))?;
            self.orientation_sign = Some(first_index.0 * second_index.0 * new_sign);
            Some(*new_plane)
        } else {
            None
        };
        let command = match &line.command {
            Some(CommandContent::LinearMove(LinearMove(target))) => Some(CommandContent::LinearMove(LinearMove(self.transformation.transform(target)))),
            Some(CommandContent::ProbeMove(ProbeMove(mode, target))) => Some(CommandContent::ProbeMove(ProbeMove(*mode, self.transformation.transform(target)))),
            Some(CommandContent::HelicalMove(HelicalMove { orientation, target, center })) => {
                let orientation_sign = self.orientation_sign.ok_or(CommandTransformError::UnknownOrientationSign)?;
                Some(CommandContent::HelicalMove(HelicalMove {
                    orientation: orientation_sign.apply(*orientation),
                    target: self.transformation.transform(target),
                    center: self.transformation.transform(center)
                }))
            },
            None => None,
        };
        Ok(Line {
            modal_updates: ModalUpdates {
                arc_plane,
                ..line.modal_updates
            },
            command,
        })
    }
}

pub fn transform_gcode_file(
    config: &MachineConfiguration,
    transform: &SimpleTransform,
    input: &str,
) -> Result<String, usize> {
    let mut transformer = CommandTransformer::new(
        transform,
        config.arc_planes.iter().map(|x| ArcPlane(x.first_axis, x.second_axis)).collect()
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


