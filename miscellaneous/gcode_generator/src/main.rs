// Probably need to install libgeos-dev from ppa:ubuntugis/ppa for this to work.

mod region_to_spiral_path;
mod lines;
mod collection;
mod onion;
mod spiral_path;
mod comparable_float;
mod multitool_path;
mod gcode;
mod smooth_region;
mod debug_show;

use std::{fs, process, mem, collections::HashMap};

use clap::Parser;
use geos::{Geom, Geometry, CoordSeq};
use itertools::chain;
use multitool_path::{ForegroundCutInfo, CuttingStep};
use smooth_region::sequence_cuts_non_bleeding;
use spiral_path::{SpiralConfiguration, MillingMode};
use serde::{Deserialize, Serialize};
use tempfile::tempdir;

use debug_show::show_geometries;

use crate::{onion::OnionTree, gcode::{stroke_paths, paths_to_coordinates, StrokeOptions, reflect_paths}};

#[derive(Deserialize)]
struct WKTRow {
    label: Option<String>,
    wkt: String
}


// Given a geometry, return a list of polygons, from largest to smallest, of all offsets at a multiple of offset_size until the result is contained in minimum.


#[derive(Parser, Debug)]
struct Arguments {
    input: String,
    output: String,
    names: Vec<String>,

    #[arg(long)]
    no_reverse: bool,
}

struct NamedGeometry<'a> {
    name: String,
    geometry: Geometry<'a>
}

fn main() {
    //let input = fs::read_to_string("../svg2wkt/practice_plain.json").expect("Failed to read file");
    let arguments = Arguments::parse();
    fs::create_dir_all(&arguments.output).expect("Failed to create output directory");
    let input = fs::read_to_string(&arguments.input).expect("Failed to read file");

    let values: Vec<WKTRow> = serde_json::from_str(&input).expect("Bad json!");

    let mut geometries = Vec::new();
    let mut name_to_geometry = HashMap::new();

    for value in values {
        if let Some(label) = value.label {
            name_to_geometry.insert(
                label,
                geos::Geometry::new_from_wkt(&value.wkt).and_then(|geo| geo.make_valid()).expect("Bad WKT")
            );
        }
    }
    for name in &arguments.names {
        match name_to_geometry.get(name) {
            Some(geometry) => geometries.push(Clone::clone(geometry)),
            None => panic!("{} not found!", name),
        }
    }

    let result = sequence_cuts_non_bleeding(
        geometries,
        25.4/80.0,
        16
    ).unwrap();

    /* for path in &result {
        show_geometries(&vec![Clone::clone(path)]);
    }*/
    // show_geometries(&result);
    let mut total = Geometry::create_multipolygon(Vec::new()).unwrap();
    for item in &result {
        total = total.union(item).unwrap();
    }
    // show_geometries(&vec![total]);

    let very_coarse_info = CuttingStep {
        tool_radius: 25.4/8.0*3.0,
        step_over: 25.4/8.0*3.0,
        safety_margin: 25.4/8.0,
        simplification_tolerance: 0.1,
        quadsegs: 16,
        milling_mode: MillingMode::Climb,
        profile_pass: false,
    };


    let coarse_info = CuttingStep {
        tool_radius: 25.4/16.0,
        step_over: 25.4/16.0,
        safety_margin: 25.4/160.0,
        simplification_tolerance: 0.025,
        quadsegs: 16,
        milling_mode: MillingMode::Climb,
        profile_pass: false,
    };
    let fine_info = CuttingStep {
        tool_radius: 25.4/80.0,
        step_over: 25.4/80.0,
        safety_margin: -25.4/200.0,
        simplification_tolerance: 0.025,
        quadsegs: 16,
        milling_mode: MillingMode::Climb,
        profile_pass: true,
    };
    let facing_stroke = StrokeOptions {
        safe_height: 3.0,
        feedrate: 2000.0,
        z_max: 0.0,
        z_min: 0.0,
        z_step: 1.7,
    };
    let coarse_stroke = StrokeOptions {
        safe_height: 3.0,
        feedrate: 2000.0,
        z_max: 0.0,
        z_min: -25.4/32.0 * 1.0,
        z_step: 4.0,
    };
    let fine_stroke = StrokeOptions {
        safe_height: 3.0,
        feedrate: 700.0,
        z_max: 0.0,
        z_min: -25.4/32.0 * 1.0,
        z_step: 0.61,
    };


    // Foreground cut
    for (index, item) in result.iter().enumerate() {
        let name = &arguments.names[index];

        let reflect_if_needed = |paths| if index == 0 || arguments.no_reverse {
            paths
        } else {
            reflect_paths(paths)
        };
        if index > 0 {
            // REVERSE POLARITY! Necessary because we're reflecting these paths.
            // It's possible the roughing step ought to be climb milling for real; the
            // fine step might be sensitive to deflection given the tiny bit, so probably
            // better to do with conventional milling
            let coarse_info = CuttingStep {
                milling_mode: MillingMode::Conventional,
                ..coarse_info
            };
            let fine_info = CuttingStep {
                milling_mode: MillingMode::Conventional,
                ..fine_info
            };        
        }

        let circle = geos::Geometry::create_point(CoordSeq::new_from_vec(&[&[22.5, 22.5]]).unwrap()).unwrap().buffer(22.49, 64).unwrap();

        let target_region = item.envelope().unwrap().buffer(25.4*0.125, 16).unwrap().intersection(&circle).unwrap();
        let total_region = item.envelope().unwrap().buffer(25.4*0.25, 16).unwrap().intersection(&circle).unwrap();
        let required_region = target_region.difference(item).unwrap();
        let allowed_region = total_region.difference(item).unwrap();
    
    
        let mut foreground_cut = ForegroundCutInfo {
            required_region: &required_region,
            allowed_region: &allowed_region,
            cut_region: Geometry::create_multipolygon(Vec::new()).unwrap()
        };

        let allowed_facing = item.buffer(10.0, 16).unwrap();
        let mut facing_cut = ForegroundCutInfo {
            required_region: item,
            allowed_region: &allowed_facing,
            cut_region: Geometry::create_multipolygon(Vec::new()).unwrap()
        };

        let facing_step = facing_cut.add_step(&coarse_info).unwrap();
        let very_coarse_step = foreground_cut.add_step(&very_coarse_info).unwrap();
        let coarse_step = foreground_cut.add_step(&coarse_info).unwrap();
        let fine_step = foreground_cut.add_step(&fine_info).unwrap();

        let facing_gcode = stroke_paths(&facing_stroke, &reflect_if_needed(paths_to_coordinates(&facing_step).unwrap()));
        let very_coarse_gcode = stroke_paths(&coarse_stroke, &reflect_if_needed(paths_to_coordinates(&very_coarse_step).unwrap()));
        let coarse_gcode = stroke_paths(&coarse_stroke, &reflect_if_needed(paths_to_coordinates(&coarse_step).unwrap()));
        let fine_gcode = stroke_paths(&fine_stroke, &reflect_if_needed(paths_to_coordinates(&fine_step).unwrap()));


        fs::write(format!("{}/{}_very_coarse_path.nc", arguments.output, name), very_coarse_gcode).unwrap();
        fs::write(format!("{}/{}_coarse_path.nc", arguments.output, name), coarse_gcode).unwrap();
        fs::write(format!("{}/{}_fine_path.nc", arguments.output, name), fine_gcode).unwrap();
    }

}
