use std::{collections::HashMap, hash::Hash};

use ndarray::Axis;

use super::{GCodeCommand, GCodeLine, AxisValues};

fn get_position_from_command<'a>(command: &'a GCodeCommand) -> Option<&'a AxisValues> {
    match command {
        GCodeCommand::Move { mode, position } => Some(position),
        GCodeCommand::Probe { position, mode, requirement } => Some(position),
        GCodeCommand::ArcMove { orientation, position, offsets, revolutions} => Some(position),
        _ => None
    }
}

pub fn map_to_axis_values(map: &HashMap<usize, f64>) -> AxisValues {
    AxisValues(map.iter().map(|(index, value)| (*index, *value)).collect())
}

pub fn get_first_position<'a>(program: impl IntoIterator<Item=&'a GCodeLine>) -> AxisValues {
    let mut values = HashMap::new();
    for gcode in program {
        let position = gcode.command.as_ref().and_then(get_position_from_command);
        if let Some(position) = position {
            for (coord, value) in &position.0 {
                if !values.contains_key(coord) {
                    values.insert(*coord, *value);
                }
            }
        }
    }
    map_to_axis_values(&values)
}

#[derive(Debug)]
pub struct BadAxis(usize);

// Guarunteed to be homogenous...
pub fn as_lines_simple<'a>(program: impl IntoIterator<Item=&'a GCodeLine>, start: AxisValues) -> Result<Vec<AxisValues>, BadAxis> {
    let mut last_position: HashMap<_, _> = start.0.into_iter().collect();
    let mut result = vec![map_to_axis_values(&last_position)];
    for gcode in program {
        let position = gcode.command.as_ref().and_then(get_position_from_command);
        if let Some(position) = position {
            for (coord, value) in &position.0 {
                let old = last_position.insert(*coord, *value);
                if old.is_none() {
                    return Err(BadAxis(*coord));
                }
            }
            result.push(map_to_axis_values(&last_position));
        }
    }
    Ok(result)
}

pub fn as_lines_from_best_start<'a>(program: impl IntoIterator<Item=&'a GCodeLine> + Copy) -> Vec<AxisValues> {
    // Cannot fail because we check through same program twice.
    as_lines_simple(program, get_first_position(program)).unwrap()
}