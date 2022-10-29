use {
    super::{
        AxisValues, CoordinateMode, CoordinateSystem, GCodeCommand, GCodeFormatSpecification,
        GCodeLine, GCodeModal, MoveMode, OffsetAxisValues, Orientation, Plane, ProbeDirection,
        ProbeRequirement, SpindleMode, Unit,
    },
    std::fmt::Display,
};

struct GCodeAxisPrinter<'a> {
    float_digits: usize,
    axis_letters: &'a Vec<u8>,
    axis_info: &'a Vec<(usize, f64)>,
}
impl<'a> Display for GCodeAxisPrinter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut is_first = true;
        for (index, value) in self.axis_info {
            if is_first {
                is_first = false;
            } else {
                write!(f, " ")?;
            }
            write!(
                f,
                "{}{:.2$}",
                self.axis_letters[*index] as char, value, self.float_digits
            )?;
        }
        Ok(())
    }
}
struct GCodeModalPrinter<'a> {
    float_digits: usize,
    modal: &'a GCodeModal,
}
impl<'a> Display for GCodeModalPrinter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.modal {
            GCodeModal::SetFeedrate(value) => write!(f, "F{:.1$}", value, self.float_digits),
            GCodeModal::SetArcPlane(Plane::XY) => write!(f, "G17"),
            GCodeModal::SetArcPlane(Plane::ZX) => write!(f, "G18"),
            GCodeModal::SetArcPlane(Plane::YZ) => write!(f, "G19"),
            GCodeModal::SetUnits(Unit::Inch) => write!(f, "G20"),
            GCodeModal::SetUnits(Unit::Millimeter) => write!(f, "G21"),
            GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord0) => write!(f, "G54"),
            GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord1) => write!(f, "G55"),
            GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord2) => write!(f, "G56"),
            GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord3) => write!(f, "G57"),
            GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord4) => write!(f, "G58"),
            GCodeModal::SetCoordinateSystem(CoordinateSystem::Coord5) => write!(f, "G59"),
            GCodeModal::SetCoordinateMode(CoordinateMode::Absolute) => write!(f, "G90"),
            GCodeModal::SetCoordinateMode(CoordinateMode::Incremental) => write!(f, "G91"),
            GCodeModal::SetSpindle(SpindleMode::Clockwise) => write!(f, "M3"),
            GCodeModal::SetSpindle(SpindleMode::Off) => write!(f, "M5"),
            GCodeModal::SetSpindleSpeed(speed) => write!(f, "S{:.1$}", speed, self.float_digits),
            GCodeModal::EndProgram => write!(f, "M2"),
        }
    }
}
struct GCodeCommandPrinter<'a> {
    settings: &'a GCodeFormatSpecification,
    command: &'a GCodeCommand,
}
impl<'a> Display for GCodeCommandPrinter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.command {
            GCodeCommand::Move { mode, position } => {
                match mode {
                    MoveMode::Rapid => write!(f, "G0 ")?,
                    MoveMode::Controlled => write!(f, "G1 ")?,
                    MoveMode::Unspecified => (),
                };
                self.settings.format_axes(position).fmt(f)
            }
            GCodeCommand::ArcMove {
                orientation,
                position,
                offsets,
                revolutions,
            } => {
                match orientation {
                    Orientation::Clockwise => write!(f, "G2 ")?,
                    Orientation::Counterclockwise => write!(f, "G3 ")?,
                };
                write!(
                    f,
                    "{} {}",
                    self.settings.format_axes(position),
                    self.settings.format_offset_axes(offsets)
                )?;
                if let Some(revolutions) = revolutions {
                    write!(f, "P{}", revolutions)?;
                }
                Ok(())
            }
            GCodeCommand::Dwell { duration } => {
                write!(f, "G4 P{:.1$}", duration, self.settings.float_digits)
            }
            GCodeCommand::SetWorkCoordinateTo(coordinates) => {
                write!(f, "G10 L20 {}", self.settings.format_axes(coordinates))
            }
            GCodeCommand::Probe {
                position,
                mode,
                requirement,
            } => {
                let subcommand = match (mode, requirement) {
                    (ProbeDirection::Towards, ProbeRequirement::Require) => "G38.2",
                    (ProbeDirection::Towards, ProbeRequirement::Optional) => "G38.3",
                    (ProbeDirection::Away, ProbeRequirement::Require) => "G38.4",
                    (ProbeDirection::Away, ProbeRequirement::Optional) => "G38.5",
                };
                write!(f, "{} {}", subcommand, self.settings.format_axes(position))
            }
        }
    }
}
struct GCodeLinePrinter<'a> {
    settings: &'a GCodeFormatSpecification,
    line: &'a GCodeLine,
}
impl<'a> Display for GCodeLinePrinter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.line.modals.is_empty() {
            let mut is_first = true;
            for modal in &self.line.modals {
                if is_first {
                    is_first = false;
                } else {
                    write!(f, " ")?;
                }
                write!(f, "{}", self.settings.format_modal(modal))?;
            }
            if self.line.command.is_some() {
                write!(f, " ")?;
            }
        }
        if let Some(command) = &self.line.command {
            write!(f, "{}", self.settings.format_command(command))?;
        }
        Ok(())
    }
}
impl GCodeFormatSpecification {
    fn format_axes<'a>(&'a self, values: &'a AxisValues) -> impl Display + 'a {
        GCodeAxisPrinter {
            float_digits: self.float_digits,
            axis_letters: &self.axis_letters,
            axis_info: &values.0,
        }
    }
    fn format_offset_axes<'a>(&'a self, values: &'a OffsetAxisValues) -> impl Display + 'a {
        GCodeAxisPrinter {
            float_digits: self.float_digits,
            axis_letters: &self.offset_axis_letters,
            axis_info: &values.0,
        }
    }
    fn format_modal<'a>(&'a self, modal: &'a GCodeModal) -> impl Display + 'a {
        GCodeModalPrinter {
            float_digits: self.float_digits,
            modal,
        }
    }
    fn format_command<'a>(&'a self, command: &'a GCodeCommand) -> impl Display + 'a {
        GCodeCommandPrinter {
            settings: self,
            command,
        }
    }
    pub fn format_line<'a>(&'a self, line: &'a GCodeLine) -> impl Display + 'a {
        GCodeLinePrinter {
            settings: self,
            line,
        }
    }
}

// Ignored: G40, G43, G49, G91.1, G92, G93, G53

#[cfg(test)]
mod test {
    use {super::*, std::string::ToString};

    fn default_settings() -> GCodeFormatSpecification {
        GCodeFormatSpecification {
            axis_letters: b"XYZA".to_vec(),
            offset_axis_letters: b"IJK".to_vec(),
            float_digits: 2,
        }
    }
    fn line_to_string(line: &GCodeLine) -> String {
        default_settings().format_line(line).to_string()
    }

    #[test]
    fn test_simple_line() {
        let line = GCodeLine {
            modals: vec![GCodeModal::SetFeedrate(1000.0)],
            command: Some(GCodeCommand::Move {
                mode: MoveMode::Controlled,
                position: AxisValues(vec![(0, 50.002), (1, 12.5)]),
            }),
        };
        assert_eq!(line_to_string(&line), "F1000.00 G1 X50.00 Y12.50");
    }
    #[test]
    fn test_no_modal_line() {
        let line = GCodeLine {
            modals: vec![],
            command: Some(GCodeCommand::Move {
                mode: MoveMode::Rapid,
                position: AxisValues(vec![(0, 0.0), (2, -1.0 / 3.0)]),
            }),
        };
        assert_eq!(line_to_string(&line), "G0 X0.00 Z-0.33");
    }
    #[test]
    fn test_no_command_line() {
        let line = GCodeLine {
            modals: vec![GCodeModal::SetUnits(Unit::Millimeter)],
            command: None,
        };
        assert_eq!(line_to_string(&line), "G21");
    }
    #[test]
    fn test_multiple_modal() {
        let line = GCodeLine {
            modals: vec![
                GCodeModal::SetUnits(Unit::Millimeter),
                GCodeModal::SetArcPlane(Plane::YZ),
            ],
            command: None,
        };
        assert_eq!(line_to_string(&line), "G21 G19");
    }
}
