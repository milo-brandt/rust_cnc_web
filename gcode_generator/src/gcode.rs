use geos::{Geom, Geometry};
use itertools::Itertools;

use crate::lines::coord_seq_to_vec;

pub struct StrokeOptions {
    pub safe_height: f64,
    pub feedrate: f64,
    pub z_max: f64,
    pub z_min: f64,
    pub z_step: f64,
}

struct RangeIterator {
    z_last: f64,
    z_min: f64,
    z_step: f64,
}
impl Iterator for RangeIterator {
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.z_last < self.z_min {
            None
        } else {
            self.z_last -= self.z_step;
            if self.z_last <= self.z_min {
                self.z_last = self.z_min - 1.0; // ensure next loop detects that we're done.
                Some(self.z_min)
            } else {
                Some(self.z_last)
            }
        }
    }
}
pub fn z_steps(z_max: f64, z_min: f64, z_step: f64) -> impl Iterator<Item=f64> {
    RangeIterator {
        z_last: z_max,
        z_min,
        z_step
    }
}

pub fn paths_to_coordinates(paths: &Vec<Geometry>) -> geos::GResult<Vec<Vec<[f64; 2]>>> {
    paths.iter().map(|geometry| geometry.get_coord_seq().and_then(|coord_seq| coord_seq_to_vec(&coord_seq))).collect()
}
pub fn reflect_paths_in_place(paths: &mut Vec<Vec<[f64; 2]>>) {
    for path in paths {
        for item in path {
            item[0] = -item[0]
        }
    }
}
pub fn reflect_paths(mut paths: Vec<Vec<[f64; 2]>>) -> Vec<Vec<[f64; 2]>> {
    reflect_paths_in_place(&mut paths);
    paths
}
pub fn stroke_paths(options: &StrokeOptions, paths: &Vec<Vec<[f64; 2]>>) -> String {
    let main_gcode = z_steps(options.z_max, options.z_min, options.z_step).map(|depth| {
        paths.iter().map(|path| {
            let body = path.iter().map(|[x, y]| {
                format!("G1 X{:.3} Y{:.3} Z{:.3} F{:.3}", x, y, depth, options.feedrate)
            }).join("\n");
            format!("G1 Z{} F{}\nG0 X{} Y{}\n{}",
                options.safe_height,
                options.feedrate,
                path[0][0],
                path[0][1],
                body
            )
        }).join("\n\n")
    }).join("\n\n\n");
    format!("{}\nG1 Z{} F{}", main_gcode, options.safe_height, options.feedrate)
}