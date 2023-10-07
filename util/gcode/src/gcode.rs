use std::{collections::HashMap, ops::Neg};

use crate::{probe::ProbeMode, coordinates::{PartialPosition, PartialOffset}, transform::{SimpleTransform, Transform, TryTransform, Sign}};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ArcPlane(pub u8, pub u8);
impl ArcPlane {
    pub fn compare(lhs: &ArcPlane, rhs: &ArcPlane) -> Option<Sign> {
        if lhs.0 == rhs.0 && lhs.1 == rhs.1 {
            Some(Sign::Positive)
        } else if lhs.1 == rhs.0 && lhs.0 == rhs.1 {
            Some(Sign::Negative)
        } else {
            None
        }
    }
}
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CoordinateMode { Absolute } // or Incremental - unsupported
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Units { Millimeters } // or inches - unsupported
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MotionMode { Controlled, Rapid }
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ModalUpdates {
    pub feedrate: Option<f64>,
    pub motion_mode: Option<MotionMode>,
    pub coordinate_mode: Option<CoordinateMode>,
    pub units: Option<Units>,
    pub arc_plane: Option<ArcPlane>,
}


pub struct LinearMove(pub PartialPosition);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Orientation { Clockwise, Counterclockiwse }
impl Neg for Orientation {
    type Output = Orientation;
    fn neg(self) -> Self::Output {
        match self {
            Orientation::Clockwise => Orientation::Counterclockiwse,
            Orientation::Counterclockiwse => Orientation::Clockwise,
        }
    }
}
pub struct HelicalMove {
    pub orientation: Orientation,
    pub target: PartialPosition,
    pub center: PartialOffset,
}
pub struct ProbeMove(pub ProbeMode, pub PartialPosition);

pub enum CommandContent {
    LinearMove(LinearMove),
    HelicalMove(HelicalMove),
    ProbeMove(ProbeMove),
}
pub struct Line {
    pub modal_updates: ModalUpdates,
    pub command: Option<CommandContent>,
}

pub struct CommandTransformer {
    orientation_sign: Option<Sign>,
    planes: Vec<ArcPlane>,
    transformation: SimpleTransform,
}
pub enum CommandTransformError {
    UnknownOrientationSign,
    InvalidArcPlane(u8, u8),
}
impl CommandTransformer {
    pub fn new(transformation: SimpleTransform, planes: Vec<ArcPlane>) -> Self {
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
            self.orientation_sign = Some(new_sign);
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

pub struct ArcPlaneCommand {
    pub command: String,
    pub primary_axis: u8,
    pub secondary_axis: u8,
}
pub struct AxisConfiguration {
    pub axis_names: Vec<char>,
    pub offset_axis_names: Vec<char>,
    pub arc_planes: Vec<ArcPlaneCommand>,
}