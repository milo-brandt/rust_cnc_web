use geos::{Geom, Geometry};

pub fn to_geometry_list<'a>(geometry: &Geometry<'a>) -> geos::GResult<Vec<Geometry<'a>>> {
    let length = geometry.get_num_geometries()?;
    let mut result = Vec::with_capacity(length);
    for index in 0..length {
        result.push(geometry.get_geometry_n(index)?.clone());
    }
    Ok(result)
}
