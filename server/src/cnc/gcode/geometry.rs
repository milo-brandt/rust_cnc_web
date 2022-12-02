use std::{collections::HashMap, hash::Hash, f64::consts::{PI, FRAC_PI_2}};

use itertools::chain;
use ndarray::Axis;

use super::{GCodeCommand, GCodeLine, AxisValues, ArcPlane, GCodeModal, Orientation, OffsetAxisValues, GCodeFormatSpecification};

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
pub enum GCodePositionError {
    BadAxis(usize),
    NoArcMode,
    InvalidArc,
}

fn update_position(last_position: &mut HashMap<usize, f64>, axis_value: &AxisValues) -> Result<(), GCodePositionError> {
    for (coord, value) in &axis_value.0 {
        let old = last_position.insert(*coord, *value);
        if old.is_none() {
            return Err(GCodePositionError::BadAxis(*coord));
        }
    }
    Ok(())
}
fn arc_indices(plane: ArcPlane, orientation: Orientation) -> (usize, usize) {
    // Returns indices (i, j) such that the arc will be a CCW arc in the projection v -> (v_i, v_j)
    let (i, j) = match plane {  // First get how CCW arcs would be done...
        ArcPlane::XY => (0, 1),
        ArcPlane::ZX => (2, 0),
        ArcPlane::YZ => (1, 2),
    };
    match orientation {  // ... then swap for clockwise arcs
        Orientation::Clockwise => (j, i),
        Orientation::Counterclockwise => (i, j),
    }
}
fn get_latter_value(positions: &Vec<(usize, f64)>, index: usize) -> Option<f64> {
    for (axis_index, coord) in positions {
        if index == *axis_index {
            return Some(*coord);
        }
    }
    None
}
fn tuple_dif(x: (f64, f64), y: (f64, f64)) -> (f64, f64) {
    (x.0 - y.0, x.1 - y.1)
}
fn tuple_sum(x: (f64, f64), y: (f64, f64)) -> (f64, f64) {
    (x.0 + y.0, x.1 + y.1)
}
fn tuple_mag_sq(x: (f64, f64)) -> f64 {
    x.0 * x.0 + x.1 * x.1
}
fn tuple_mag(x: (f64, f64)) -> f64 {
    tuple_mag_sq(x).sqrt()
}
fn angle_of(x: (f64, f64)) -> f64 {
    x.1.atan2(x.0)
}

fn arc_points(start: &mut HashMap<usize, f64>, end: &AxisValues, offsets: &OffsetAxisValues, tolerance: f64, arc_indices: (usize, usize), revolutions: u64) -> Result<impl Iterator<Item=AxisValues> + 'static, GCodePositionError> {
    let position_clone = start.clone();
    let arc_start = (
        *start.get(&arc_indices.0).ok_or(GCodePositionError::BadAxis(arc_indices.0))?,
        *start.get(&arc_indices.1).ok_or(GCodePositionError::BadAxis(arc_indices.1))?,
    );
    let arc_center = (
        get_latter_value(&offsets.0, arc_indices.0).unwrap_or(0.0) + arc_start.0,
        get_latter_value(&offsets.0, arc_indices.1).unwrap_or(0.0) + arc_start.1,
    );
    let arc_end = (
        get_latter_value(&end.0, arc_indices.0).unwrap_or(arc_start.0),
        get_latter_value(&end.0, arc_indices.1).unwrap_or(arc_start.1),
    );

    let helical_axes: Vec<(usize, f64, f64)> = position_clone.iter().filter(|(axis, _)| {
        **axis != arc_indices.0 && **axis != arc_indices.1
    }).map(|(axis, start_value)| {
        match get_latter_value(&end.0, *axis) {
            Some(end_value) => (*axis, *start_value, end_value - *start_value),
            None => (*axis, *start_value, *start_value)
        }
    }).collect();

    let distance_start = tuple_mag(tuple_dif(arc_start, arc_center));
    let distance_end = tuple_mag(tuple_dif(arc_end, arc_center));
    let distance_difference = distance_end - distance_start;
    if distance_difference.abs() > 0.01 {
        return Err(GCodePositionError::InvalidArc);
    }
    let start_angle = angle_of(arc_start);
    let mut end_angle = angle_of(arc_end);
    if end_angle < start_angle {
        end_angle += 2.0*PI;
    }
    end_angle += revolutions as f64 * 2.0*PI;

    let angle_difference = end_angle - start_angle;

    // Angle must be such that (1 - cos(theta / 2)) * distance_start < tolerance
    // i.e.: theta < cos^-1(1 - tolerance / distance_start) * 2
    let step_count = if 2.0 * distance_start > tolerance {
        let max_step = ((1.0 - tolerance / distance_start).acos() * 2.0).min(FRAC_PI_2);
        (angle_difference / max_step).ceil() as u64 + 1
    } else {
        1
    };


    let get_step = move |step: u64| {
        let progress = step as f64 / step_count as f64;
        let angle = start_angle + progress * angle_difference;
        let distance = distance_start + progress * distance_difference;
        let arc_position = tuple_sum(arc_center, (angle.cos() * distance, angle.sin() * distance));
        AxisValues(
            chain(
                [
                    (arc_indices.0, arc_position.0),
                    (arc_indices.1, arc_position.1),
                ],
                helical_axes.iter().map(|(axis, start, dif)| (*axis, *start + progress * *dif))
            ).collect()
        )
    };
    let axis_start = map_to_axis_values(start);
    update_position(start, end)?;
    let axis_end = map_to_axis_values(start);
    Ok(chain!(
        [axis_start],
        (1..step_count).map(get_step),
        [axis_end]
    ))
}

// Guarunteed to be homogenous...
pub fn as_lines_simple<'a>(program: impl IntoIterator<Item=&'a GCodeLine>, start: AxisValues, mut arc_mode: Option<ArcPlane>) -> Result<Vec<AxisValues>, GCodePositionError> {
    let mut last_position: HashMap<_, _> = start.0.into_iter().collect();
    let mut result = vec![map_to_axis_values(&last_position)];
    for gcode in program {
        for modal in &gcode.modals {
            if let GCodeModal::SetArcPlane(arc_plane) = modal {
                arc_mode = Some(*arc_plane);
            }
        }
        match &gcode.command {
            Some(GCodeCommand::Move { mode, position }) => {
                update_position(&mut last_position, position);
                result.push(map_to_axis_values(&last_position));
            },
            Some(GCodeCommand::Probe { mode, position, requirement }) => {
                update_position(&mut last_position, position);
                result.push(map_to_axis_values(&last_position));
            },
            Some(GCodeCommand::ArcMove { orientation, position, offsets, revolutions }) => {
                if arc_mode.is_none() { 
                    return Err(GCodePositionError::NoArcMode);
                }
                let indices = arc_indices(arc_mode.unwrap(), *orientation);
                let points = arc_points(&mut last_position, position, offsets, 0.1, indices, revolutions.unwrap_or(0))?;
                result.extend(points);                
            }
            _ => (),
        }
        let position = gcode.command.as_ref().and_then(get_position_from_command);
        if let Some(position) = position {
            result.push(map_to_axis_values(&last_position));
        }
    }
    Ok(result)
}

pub fn as_lines_from_best_start<'a>(program: impl IntoIterator<Item=&'a GCodeLine> + Copy) -> Result<Vec<AxisValues>, GCodePositionError> {
    // Cannot fail because we check through same program twice.
    as_lines_simple(program, get_first_position(program), Some(ArcPlane::XY))
}