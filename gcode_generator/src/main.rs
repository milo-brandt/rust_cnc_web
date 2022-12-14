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

use std::{fs, process};

use geos::{Geom, Geometry, CoordSeq};
use itertools::chain;
use multitool_path::{ForegroundCutInfo, CuttingStep};
use smooth_region::sequence_cuts_non_bleeding;
use spiral_path::{SpiralConfiguration, MillingMode};
use serde::{Deserialize, Serialize};
use tempfile::tempdir;

use crate::{onion::OnionTree, gcode::{stroke_paths, paths_to_coordinates, StrokeOptions, reflect_paths}};

#[derive(Deserialize)]
struct WKTRow {
    label: Option<String>,
    wkt: String
}


#[derive(Serialize)]
#[serde(tag = "type")]
enum Element {
    Polygon{
        wkt: String
    },
    Line{
        wkt: String
    }
}
impl Element {
    pub fn from_polygon(geo: &Geometry) -> Self {
        Element::Polygon{
            wkt: geo.to_wkt().unwrap()
        }
    }
    pub fn from_line(geo: &Geometry) -> Self {
        Element::Line{
            wkt: geo.to_wkt().unwrap()
        }
    }
}

fn show_geometry(geometry: &Vec<Element>) {
    let tmp_dir = tempdir().unwrap();
    let file_path = tmp_dir.path().join("rendering.json");
    fs::write(&file_path, serde_json::to_string(geometry).unwrap()).unwrap();

    process::Command::new("python3").arg("show_wkt.py").arg(file_path).spawn().and_then(|mut child| child.wait()).unwrap();
}

// Given a geometry, return a list of polygons, from largest to smallest, of all offsets at a multiple of offset_size until the result is contained in minimum.


fn main() {
    let input = fs::read_to_string("../svg2wkt/practice_plain.json").expect("Failed to read file");

    let values: Vec<WKTRow> = serde_json::from_str(&input).expect("Bad json!");

    let mut geometries = Vec::new();

    for value in &values {
        geometries.push(
            geos::Geometry::new_from_wkt(&value.wkt).and_then(|geo| geo.make_valid()).expect("Bad WKT")
        );
    }

    let gg1 = geos::Geometry::new_from_wkt("POLYGON ((0 0, 0 5, 6 6, 6 1, 10 1, 10 4, 14 4, 14 0, 0 0))")
    .expect("invalid WKT");
    let gg2 = geos::Geometry::new_from_wkt("POLYGON ((1 1, 1 3, 5 5, 5.5 0.5, 1 1))")
        .expect("invalid WKT");
    let mut gg3 = gg1.difference(&gg2).expect("difference failed");
    // normalize is only used for consistent ordering of vertices
    gg3.normalize().expect("normalize failed");


    //let layers = region_to_spiral_path::onion_layers(&gg3, 0.02, 16).unwrap();

    /*
    let projected = gg1.boundary().unwrap().project(&Geometry::create_point(CoordSeq::new_from_vec(&[[0.0, 10.0]]).unwrap()).unwrap()).unwrap();


    let cut = lines::cut_line_at(&gg1.boundary().unwrap(), projected).unwrap();

    show_geometry(&
        (&cut).into_iter().map(Element::from_line).collect()
    );

    let repositioned = lines::reposition_linear_ring_to(&gg1.boundary().unwrap(), 8.0).unwrap();

    show_geometry(&vec![Element::from_line(&repositioned)])
*/
    /*let trees = onion::onion_tree(&gg3, 0.07, 16, 0.001).unwrap();
    let mut items = Vec::new();
    fn append_items(elements: &mut Vec<Element>, tree: &OnionTree) {
        println!("ENTERING ELEMENT!");
        elements.push(Element::from_polygon(&tree.polygon));
        for subtree in &tree.children {
            append_items(elements, subtree);
            assert!(tree.polygon.contains(&subtree.polygon).unwrap());
        }
        println!("LEAVING ELEMENT!");
    }
    for tree in &trees {
        append_items(&mut items, tree)
    }*/
    let gg4 = Geom::clone(&gg3); //gg1.buffer(0.5, 16).unwrap().difference(&gg2).unwrap();

    let gg3 = Clone::clone(&geometries[1]);
    let gg4 = geometries[1].buffer(4.0, 16).unwrap().difference(&geometries[0]).unwrap();


    
    /*let result = cut_from_allowable_region(
        &SpiralConfiguration {
            step_over: 0.041,
            milling_mode: spiral_path::MillingMode::Normal,
            simplification_tolerance: 0.01,
            quadsegs: 16,
        },
        &gg3
    ).unwrap();*/
    let show = |paths: &Vec<Geometry>| {
        show_geometry(&paths.iter().map(|geo| Element::from_polygon(geo)).collect());
    };

    let coarse_info = CuttingStep {
        tool_radius: 25.4/16.0,
        step_over: 25.4/16.0,
        safety_margin: 25.4/80.0,
        simplification_tolerance: 0.025,
        quadsegs: 16,
        milling_mode: MillingMode::Climb,
        profile_pass: false,
    };
    let fine_info = CuttingStep {
        tool_radius: 25.4/80.0,
        step_over: 25.4/80.0,
        safety_margin: 0.0,
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
        z_min: -3.0,
        z_step: 1.7,
    };
    let fine_stroke = StrokeOptions {
        safe_height: 3.0,
        feedrate: 700.0,
        z_max: 0.0,
        z_min: -3.0,
        z_step: 0.51,
    };

    let result = sequence_cuts_non_bleeding(
        vec![
            Clone::clone(&geometries[1]),
            Clone::clone(&geometries[0]),
        ],
        25.4/80.0,
        16
    ).unwrap();

    //show(&result);
    

    // Background cut
    {
        let required_region = Clone::clone(&result[1]);
        let allowed_region = Clone::clone(&result[1]);
    
        let mut foreground_cut = ForegroundCutInfo {
            required_region: &required_region,
            allowed_region: &allowed_region,
            cut_region: Geometry::create_multipolygon(Vec::new()).unwrap()
        };

        let allowed_facing = geometries[1].buffer(10.0, 16).unwrap();
        let mut facing_cut = ForegroundCutInfo {
            required_region: &result[0],
            allowed_region: &allowed_facing,
            cut_region: Geometry::create_multipolygon(Vec::new()).unwrap()
        };

        let facing_step = facing_cut.add_step(&coarse_info).unwrap();
        let coarse_step = foreground_cut.add_step(&coarse_info).unwrap();
        let fine_step = foreground_cut.add_step(&fine_info).unwrap();

        let facing_gcode = stroke_paths(&facing_stroke, &paths_to_coordinates(&facing_step).unwrap());
        let coarse_gcode = stroke_paths(&coarse_stroke, &paths_to_coordinates(&coarse_step).unwrap());
        let fine_gcode = stroke_paths(&fine_stroke, &paths_to_coordinates(&fine_step).unwrap());

        fs::write("inlay_tree_background_fine_path.nc", fine_gcode).unwrap();
        fs::write("inlay_tree_background_coarse_path.nc", format!("{}\n\n\n\n{}", facing_gcode, coarse_gcode)).unwrap();
    }
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
    // Foreground cut
    {
        let target_region = result[1].envelope().unwrap().buffer(25.4*0.25, 16).unwrap();
        let total_region = result[1].envelope().unwrap().buffer(25.4*0.5, 16).unwrap();
        let required_region = target_region.difference(&geometries[0]).unwrap();
        let allowed_region = total_region.difference(&geometries[0]).unwrap();
    
    
        let mut foreground_cut = ForegroundCutInfo {
            required_region: &required_region,
            allowed_region: &allowed_region,
            cut_region: Geometry::create_multipolygon(Vec::new()).unwrap()
        };

        let allowed_facing = geometries[0].buffer(10.0, 16).unwrap();
        let mut facing_cut = ForegroundCutInfo {
            required_region: &result[1],
            allowed_region: &allowed_facing,
            cut_region: Geometry::create_multipolygon(Vec::new()).unwrap()
        };

        let facing_step = facing_cut.add_step(&coarse_info).unwrap();
        let coarse_step = foreground_cut.add_step(&coarse_info).unwrap();
        let fine_step = foreground_cut.add_step(&fine_info).unwrap();

        let facing_gcode = stroke_paths(&facing_stroke, &reflect_paths(paths_to_coordinates(&facing_step).unwrap()));
        let coarse_gcode = stroke_paths(&coarse_stroke, &reflect_paths(paths_to_coordinates(&coarse_step).unwrap()));
        let fine_gcode = stroke_paths(&fine_stroke, &reflect_paths(paths_to_coordinates(&fine_step).unwrap()));

        fs::write("inlay_tree_foreground_fine_path.nc", fine_gcode).unwrap();
        fs::write("inlay_tree_foreground_coarse_path.nc", format!("{}\n\n\n\n{}", facing_gcode, coarse_gcode)).unwrap();
    }

}
