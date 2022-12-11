use geos::{Geom, Geometry};

pub fn to_geometry_list<'a>(geometry: &Geometry<'a>) -> geos::GResult<Vec<Geometry<'a>>> {
    (0..geometry.get_num_geometries()?)
        .map(|index| geometry.get_geometry_n(index).map(|geo| geo.clone()))
        .collect()
}
pub fn get_interior_rings<'a>(polygon: &Geometry<'a>) -> geos::GResult<Vec<Geometry<'a>>> {
    (0..polygon.get_num_interior_rings()? as u32)
        .map(|index| polygon.get_interior_ring_n(index).map(|geo| geo.clone()))
        .collect()
}
pub fn get_all_rings<'a>(polygon: &Geometry<'a>) -> geos::GResult<Vec<Geometry<'a>>> {
    let mut rings = get_interior_rings(polygon)?;
    rings.push(Geom::clone(&polygon.get_exterior_ring()?));
    Ok(rings)
}