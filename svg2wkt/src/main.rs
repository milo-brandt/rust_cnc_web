use std::{fs::read_to_string, mem};

mod svg_parser;

use clap::Parser;
use itertools::Itertools;
use roxmltree::Document;
use serde::Serialize;
use svg_parser::Loop; 

const FILENAME: &str = "/home/milo/Documents/Modelling/CuttingBoard/separated.svg";

fn tuple_diff(lhs: &(f64, f64), rhs: &(f64, f64)) -> (f64, f64) {
    (lhs.0 - rhs.0, lhs.1 - rhs.1)
}
fn cheap_angle(pt: (f64, f64)) -> f64 { // a non-trivial function R^2 \ {(0, 0)} -> R / 8Z
    if pt.1 > pt.0 {
        if pt.1 > -pt.0 {
            -pt.0 / pt.1
        } else {
            pt.1 / pt.0 + 2.0
        }
    } else {
        if pt.1 < -pt.0 {
            -pt.0 / pt.1 + 4.0
        } else {
            pt.1 / pt.0 + 6.0
        }
    }
}

fn closest_by_multiple_of_8(new_value: f64, current_value: f64) -> f64 {
    // Return the closest value of the form 8*i + new_value to current_value for integer i.
    let difference = (current_value - new_value) * 0.125;
    let lower = new_value + difference.floor() * 8.0;
    let upper = new_value + difference.ceil() * 8.0;
    if current_value - lower < upper - current_value {
        lower
    } else {
        upper
    }
}

fn winding_number(pt: &(f64, f64), pts: &Vec<(f64, f64)>) -> i64 {
    // Number of CCW windings of pts around pt.
    let mut iterator = pts.iter();
    let start = match iterator.next() {
        Some(exterior) => cheap_angle(tuple_diff(&exterior, &pt)),
        None => return 0
    };
    let mut current = start;
    for exterior in iterator {
        current = closest_by_multiple_of_8(
            cheap_angle(tuple_diff(exterior, &pt)),
            current
        );
    }
    current = closest_by_multiple_of_8(start, current); // Ensure we return!
    return ((current - start) * 0.125) as i64;
}

fn twice_area_integrand(start: &(f64, f64), end: &(f64, f64)) -> f64 {
    // Twice the area under a trapezoid from the line (start, end) to the x-axis.
    (end.0 - start.0) * (start.1 + end.1)
}

fn signed_area(pts: &Vec<(f64, f64)>) -> f64 {
    if pts.is_empty() {
        return 0.0;
    }
    let last_point = [pts[0]];
    let cyclic = itertools::chain(pts, &last_point);
    let mut area : f64 = 0.0;
    for (last, next) in cyclic.tuple_windows() {
        area += twice_area_integrand(last, next);
    }
    area * 0.5
}

pub struct ComparableFloat(pub f64);

impl PartialEq for ComparableFloat {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl PartialOrd for ComparableFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Eq for ComparableFloat {}

impl Ord for ComparableFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.partial_cmp(&other.0).unwrap()
    }
}

#[derive(Debug)]
pub struct Polygon {
    exterior: Vec<(f64, f64)>,
    holes: Vec<Vec<(f64, f64)>>,
}

fn points_to_list(pts: &Vec<(f64, f64)>) -> String {
    let first_pt = pts[0];
    format!(
        "({})",
        itertools::chain(
            pts.iter(),
            &[first_pt]
        ).map(|(x,y)| format!("{:.6} {:.6}", x, y)).join(", ")
    )
}

fn to_wkt_polygon_unwrapped(poly: &Polygon) -> String {
    format!(
        "({})", 
        itertools::chain(
            [points_to_list(&poly.exterior)],
            poly.holes.iter().map(points_to_list)
        ).join(", ")
    )
}

fn to_wkt_polygons(polys: &Vec<Polygon>) -> String {
    if polys.len() == 0 {
        panic!("Oh no! there should be some polygons")
    } else if polys.len() == 1 {
        format!("POLYGON {}", to_wkt_polygon_unwrapped(&polys[0]))
    } else {
        format!("MULTIPOLYGON ({})", polys.iter().map(to_wkt_polygon_unwrapped).join(", "))
    }
}

fn loops_to_polygons_from_sorted(mut loops: Vec<Vec<(f64, f64)>>) -> Vec<Polygon> {
    // The allocations in here can be avoided. We're not operating on big sets, so it's fine.
    let mut result = Vec::new();
    loop {
        // Find the largest remaining loop; it must be on the outside.
        let exterior = match loops.pop() {
            Some(exterior) => exterior,
            None => return result
        };
        println!("CONSIDER AREA OF {}", signed_area(&exterior));
        // Now, iterate through all loops in reverse order of size; if any are inside, they are probably holes.
        let mut holes = Vec::new();
        let mut for_next_iteration = Vec::new();
        while let Some(candidate) = loops.pop() {
            if winding_number(&candidate[0], &exterior) != 0 {
                // If this is interior, find everything inside of it to use as a new start for a polygon, remove those from the current list
                // and register this as a hole.
                let (inside_hole, not_inside_hole): (Vec<Vec<(f64, f64)>>, _) = loops.into_iter().partition(|inner_candidate| {
                    winding_number(&inner_candidate[0], &candidate) != 0
                });
                let has_hole = !inside_hole.is_empty();
                if !inside_hole.is_empty() {
                    println!("INSIDE HOLE: {} {}", signed_area(&candidate), inside_hole.len());
                }
                result.extend(loops_to_polygons_from_sorted(inside_hole).into_iter());
                if !has_hole {
                    println!("DONE WITH HOLE!");
                }
                loops = not_inside_hole;
                holes.push(candidate);
            } else {
                // Otherwise, get to it on our next loop around.
                for_next_iteration.push(candidate);
            }
        }
        result.push(Polygon{
            exterior,
            holes
        });
        for_next_iteration.reverse(); // Reverse to preserve sorting invariant
        loops = for_next_iteration;
    }
}

fn loops_to_polygons(mut loops: Vec<Vec<(f64, f64)>>) -> Vec<Polygon> {
    loops.sort_by_cached_key(|key| ComparableFloat(signed_area(key).abs()));
    loops.retain(|key| !key.is_empty());
    loops_to_polygons_from_sorted(loops)
}


#[derive(Parser, Debug)]
#[command(author="Milo Brandt", about="A utility for converting from SVGs with only line elements to WKTs.")]
struct Arguments {
    /// A path to an SVG file to input
    input_file: String,

    /// A path to output a JSON file with the conversion
    output_file: String,
}

#[derive(Serialize)]
struct WKTRow {
    label: Option<String>,
    wkt: String
}

fn main() {
    let args = Arguments::parse();
    let data = read_to_string(&args.input_file).unwrap();
    let parsed_document = Document::parse(&data).unwrap();
    let mut rows = Vec::new();
    for item in parsed_document.descendants().filter(|item| {
        item.is_element() && item.tag_name().name() == "path"
    }) {
        let loops = svg_parser::parse_path(item.attribute("d").unwrap()).unwrap();
        let loops_len = loops.len();
        let loops = loops.into_iter().map(|l| l.positions).collect_vec();
        let polygons = loops_to_polygons(loops);
        /* println!("LABEL: {:?} PATHS: {:?} POLYGONS: {:?}", item.attribute(("http://www.inkscape.org/namespaces/inkscape","label")).unwrap(), loops_len, polygons.len());
        for polygon in polygons {
            println!("\tAREA: {} HOLES: {}", signed_area(&polygon.exterior), polygon.holes.len());
        } */
        for _ in 0..100 {
            println!("");
        }
        let label = item.attribute(("http://www.inkscape.org/namespaces/inkscape","label"));
        let wkt = to_wkt_polygons(&polygons);
        rows.push(WKTRow {
            label: label.map(String::from),
            wkt
        });
    }
    std::fs::write(
        args.output_file,
        serde_json::to_string_pretty(&rows).unwrap(),
    ).unwrap();
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_cheap_angle() {
        assert_eq!(cheap_angle((0.0, 1.0)), 0.0);
        assert_eq!(cheap_angle((-1.0, 1.0)), 1.0);
        assert_eq!(cheap_angle((-1.0, 0.0)), 2.0);
        assert_eq!(cheap_angle((-1.0, -1.0)), 3.0);
        assert_eq!(cheap_angle((0.0, -1.0)), 4.0);
        assert_eq!(cheap_angle((1.0, -1.0)), 5.0);
        assert_eq!(cheap_angle((1.0, 0.0)), 6.0);
        assert_eq!(cheap_angle((1.0, 1.0)), 7.0);
    }

    #[test]
    fn test_closest_multiple_of_8() {
        assert_eq!(closest_by_multiple_of_8(0.5, -0.5), 0.5);
        assert_eq!(closest_by_multiple_of_8(0.5, -9.5), -7.5);
        assert_eq!(closest_by_multiple_of_8(16.5, -9.5), -7.5);
        assert_eq!(closest_by_multiple_of_8(-7.5, 10.0), 8.5);
        assert_eq!(closest_by_multiple_of_8(0.0, 6.0), 8.0);
    }

    #[test]
    fn test_winding_number() {
        assert_eq!(winding_number(&(0.0, 0.0), &vec![
            (1.0, 0.0),
            (0.0, 1.0),
            (-1.0, 0.0),
            (0.0, -1.0),
        ]), 1);
    }
}