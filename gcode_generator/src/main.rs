// Probably need to install libgeos-dev from ppa:ubuntugis/ppa for this to work.

mod region_to_spiral_path;
mod lines;
mod collection;
mod onion;
mod spiral_path;

use std::{fs, process};

use geos::{Geom, Geometry, CoordSeq};
use serde::{Deserialize, Serialize};
use tempfile::tempdir;

use crate::onion::OnionTree;

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
    let gg2 = geos::Geometry::new_from_wkt("POLYGON ((1 1, 1 3, 5 5, 5 1, 1 1))")
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
    let trees = onion::onion_tree(&gg3, 0.07, 16, 0.001).unwrap();
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
    }
    show_geometry(&items);
    //show_geometry(&layers.iter().map(|geo| Element::from_line(&geo.boundary().unwrap())).collect());
}
