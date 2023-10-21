use std::{fmt::{Display, Formatter, self}, cell::Cell};

use crate::{config::MachineConfiguration, gcode::{Line, CommandContent, MotionMode, LinearMove, ProbeMove, HelicalMove, Orientation, ModalUpdates, CoordinateMode, Units, CoordinateSystem}, coordinates::{PartialPosition, PartialOffset, ArcPlane}, probe::{ProbeMode, ProbeDirection, ProbeExpectation}};

pub struct MachineFormatter<'a, T>(pub &'a MachineConfiguration, pub T);

impl<'a, 'b> Display for MachineFormatter<'a, &'b PartialPosition> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let needs_preceding_space = Cell::new(false);
        for (index, value) in self.1.0.iter().enumerate() {
            if let Some(value) = value {
                if needs_preceding_space.replace(true) {
                    write!(f, " ")?;
                }
                write!(f, "{}{:.*}", self.0.axis_characters[index], self.0.precision as usize, value)?;
            }
        }
        Ok(())
    }
}
impl<'a, 'b> Display for MachineFormatter<'a, &'b PartialOffset> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let needs_preceding_space = Cell::new(false);
        for (index, value) in self.1.0.iter().enumerate() {
            if let Some(value) = value {
                if needs_preceding_space.replace(true) {
                    write!(f, " ")?;
                }
                write!(f, "{}{:.*}", self.0.offset_characters[index], self.0.precision as usize, value)?;
            }
        }
        Ok(())
    }
}
// TODO: Shouldn't have expect here... should instead make constructing a MachineFormatter 
// verify what it needs to.
impl<'a, 'b> Display for MachineFormatter<'a, &'b Line> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let needs_preceding_space = Cell::new(false);
        macro_rules! write_new_term {
            ($($tokens:tt)*) => {
                {
                    if needs_preceding_space.replace(true) {
                        write!(f, " ")?;
                    }
                    write!(f, $($tokens)*)?
                }
            }
        }
        let ModalUpdates {
            feedrate,
            motion_mode,
            coordinate_mode,
            units,
            arc_plane,
            coordinate_system,
        } = &self.1.modal_updates;
        // Output coordinate system
        match coordinate_system {
            Some(CoordinateSystem::Zero) => write_new_term!("G54"),
            None => (),
        }
        // Output coordinate mode
        match coordinate_mode {
            Some(CoordinateMode::Absolute) => write_new_term!("G90"),
            None => (),
        }
        // Output units
        match units {
            Some(Units::Millimeters) => write_new_term!("G21"),
            None => (),
        }
        // Output arc_plane modal
        if let Some(arc_plane) = arc_plane {
            let command_name = &self.0.arc_planes.iter().find(|candidate|
                &ArcPlane(candidate.first_axis, candidate.second_axis) == arc_plane
            ).expect("Invalid arc plane in output!").command_index;
            write_new_term!("G{}", command_name);
        }
        // Output motion modal
        match motion_mode {
            Some(MotionMode::Controlled) => write_new_term!("G1"),
            Some(MotionMode::Rapid) => write_new_term!("G0"),
            None => (),
        }
        // Output primary command
        match &self.1.command {
            Some(CommandContent::LinearMove(LinearMove(target))) => {
                write_new_term!("{}", MachineFormatter(self.0, target));
            },
            Some(CommandContent::ProbeMove(ProbeMove(probe_mode, target))) => {
                let command_name = match probe_mode {
                    ProbeMode(ProbeDirection::Towards, ProbeExpectation::MustChange) => "G38.2",
                    ProbeMode(ProbeDirection::Towards, ProbeExpectation::MayChange) => "G38.3",
                    ProbeMode(ProbeDirection::Away, ProbeExpectation::MustChange) => "G38.4",
                    ProbeMode(ProbeDirection::Away, ProbeExpectation::MayChange) => "G38.5",
                };
                write_new_term!("{} {}", command_name, MachineFormatter(self.0, target));
            }
            Some(CommandContent::HelicalMove(HelicalMove { orientation, target, center })) => {
                let command_name = match orientation {
                    Orientation::Clockwise => "G2",
                    Orientation::Counterclockiwse => "G3"
                };
                write_new_term!("{} {} {}", command_name, MachineFormatter(self.0, target), MachineFormatter(self.0, center));
            }
            None => (),
        }
        // Output feedrate
        if let Some(feedrate) = feedrate {
            write_new_term!("F{:.*}", self.0.precision as usize, feedrate);
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::gcode::CoordinateSystem;

    use super::*;
    
    #[test]
    fn test_helix() {
        let config = MachineConfiguration::standard_4_axis();
        let result = MachineFormatter(&config, &Line {
            modal_updates: ModalUpdates {
                feedrate: Some(1000.0),
                motion_mode: None,
                coordinate_mode: Some(CoordinateMode::Absolute),
                units: Some(Units::Millimeters),
                arc_plane: Some(ArcPlane(2, 0)),
                coordinate_system: Some(CoordinateSystem::Zero),
            },
            command: Some(CommandContent::HelicalMove(HelicalMove {
                orientation: Orientation::Counterclockiwse,
                target: PartialPosition(vec![Some(1.0), Some(2.0), Some(3.0), Some(4.0)]),
                center: PartialOffset(vec![Some(5.0), Some(6.0), None, None])
            })),
        }).to_string();
        assert_eq!(
            result,
            "G54 G90 G21 G18 G3 X1.000 Y2.000 Z3.000 A4.000 I5.000 J6.000 F1000.000"
        );
    }
    #[test]
    fn test_simple() {
        let config = MachineConfiguration::standard_4_axis();
        let result = MachineFormatter(&config, &Line {
            modal_updates: ModalUpdates {
                feedrate: None,
                motion_mode: Some(MotionMode::Rapid),
                coordinate_mode: None,
                units: None,
                arc_plane: None,
                coordinate_system: None,
            },
            command: Some(CommandContent::LinearMove(LinearMove(PartialPosition(vec![Some(1.0), Some(2.0), Some(3.0), None])))),
        }).to_string();
        assert_eq!(
            result,
            "G0 X1.000 Y2.000 Z3.000"
        );
    }
}