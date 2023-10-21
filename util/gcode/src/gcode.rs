use std::{collections::HashMap, ops::Neg};

use crate::{probe::ProbeMode, coordinates::{PartialPosition, PartialOffset, Sign, ArcPlane}};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CoordinateMode { Absolute } // or Incremental - unsupported
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Units { Millimeters } // or inches - unsupported
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MotionMode { Controlled, Rapid }
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CoordinateSystem { Zero }
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ModalUpdates {
    pub feedrate: Option<f64>,
    pub motion_mode: Option<MotionMode>,
    pub coordinate_mode: Option<CoordinateMode>,
    pub units: Option<Units>,
    pub arc_plane: Option<ArcPlane>,
    pub coordinate_system: Option<CoordinateSystem>,
}


#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
pub struct HelicalMove {
    pub orientation: Orientation,
    pub target: PartialPosition,
    pub center: PartialOffset,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeMove(pub ProbeMode, pub PartialPosition);

#[derive(Debug, Clone, PartialEq)]
pub enum CommandContent {
    LinearMove(LinearMove),
    HelicalMove(HelicalMove),
    ProbeMove(ProbeMove),
}
impl CommandContent {
    pub fn target(&self) -> &PartialPosition {
        match self {
            CommandContent::LinearMove(LinearMove(target)) => target,
            CommandContent::HelicalMove(HelicalMove { target, .. }) => target,
            CommandContent::ProbeMove(ProbeMove(_, target)) => target,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub modal_updates: ModalUpdates,
    pub command: Option<CommandContent>,
}

/// A representation of the (partial) modal state of a machine, as it may or may not be known.
pub struct MachineState {
    pub feedrate: Option<f64>,
    pub motion_mode: Option<MotionMode>,
    pub coordinate_mode: Option<CoordinateMode>,
    pub units: Option<Units>,
    pub arc_plane: Option<ArcPlane>,
    pub coordinate_system: Option<CoordinateSystem>,
    pub position: PartialPosition,
}
impl MachineState {
    pub fn new(axis_count: u8) -> Self {
        Self {
            feedrate: None,
            motion_mode: None,
            coordinate_mode: None,
            units: None,
            arc_plane: None,
            coordinate_system: None,
            position: PartialPosition((0..axis_count).map(|_| None).collect())
        }
    }
    pub fn update_by(&mut self, line: &Line) {
        fn set_if_some<T: Clone>(target: &mut Option<T>, value: &Option<T>) {
            if value.is_some() {
                *target = value.clone();
            }
        }
        let ModalUpdates { 
            feedrate,
            motion_mode,
            coordinate_mode,
            units,
            arc_plane,
            coordinate_system
        } = &line.modal_updates;

        set_if_some(&mut self.feedrate, feedrate);
        set_if_some(&mut self.motion_mode, motion_mode);
        set_if_some(&mut self.coordinate_mode, coordinate_mode);
        set_if_some(&mut self.units, units);
        set_if_some(&mut self.arc_plane, arc_plane);
        set_if_some(&mut self.coordinate_system, coordinate_system);
        if let Some(target) = line.command.as_ref().map(CommandContent::target) {
            self.position.update_from(target);
        }
    }
    pub fn update_by_value(mut self, line: &Line) -> Self {
        self.update_by(line);
        self
    }
}