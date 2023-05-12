use itertools::Itertools;
use serde::{Deserialize, Serialize};
use geos::{Geometry, Geom};
use std::ops::Deref;
use std::process;
use std::fs;
use tempfile::tempdir;

use crate::collection::to_geometry_list;

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
    pub fn from_polygon<'a>(geo: &impl Geom<'a>) -> Self {
        Element::Polygon{
            wkt: geo.to_wkt().unwrap()
        }
    }
    pub fn from_line<'a>(geo: &impl Geom<'a>) -> Self {
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
pub fn show_geometries<'a, I>(geo: I)
where
    I: IntoIterator,
    <I as IntoIterator>::Item: Deref<Target=Geometry<'a>>
{
    let elements = geo.into_iter().filter_map(|geometry| {
        match geometry.geometry_type() {
            geos::GeometryTypes::Point => None,
            geos::GeometryTypes::LineString => Some(Element::from_line(&*geometry)),
            geos::GeometryTypes::LinearRing => Some(Element::from_line(&*geometry)),
            geos::GeometryTypes::Polygon => Some(Element::from_polygon(&*geometry)),
            geos::GeometryTypes::MultiPoint => None,
            geos::GeometryTypes::MultiLineString => Some(Element::from_line(&*geometry)),
            geos::GeometryTypes::MultiPolygon => Some(Element::from_polygon(&*geometry)),
            geos::GeometryTypes::GeometryCollection => None,
            _ => None,
        }
    }).collect_vec();
    show_geometry(&elements);
}