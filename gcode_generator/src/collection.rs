use geos::{Geom, Geometry, GeometryTypes};

pub fn to_geometry_list<'a>(geometry: &Geometry<'a>) -> geos::GResult<Vec<Geometry<'a>>> {
    println!("DOING: {:?}", geometry.geometry_type());
    (0..geometry.get_num_geometries()?)
        .map(|index| geometry.get_geometry_n(index).map(|geo| geo.clone()))
        .collect()
}
pub fn to_polygon_list<'a>(geometry: &Geometry<'a>) -> geos::GResult<Vec<Geometry<'a>>> {
    let mut geometries = to_geometry_list(geometry)?;
    geometries.retain(|geometry| geometry.geometry_type() == GeometryTypes::Polygon);
    Ok(geometries)
}
pub fn to_polygon_list_removing_small<'a>(geometry: &Geometry<'a>, min_radius: f64) -> geos::GResult<Vec<Geometry<'a>>> {
    let mut polygons = to_polygon_list(geometry)?;
    let mut err = None;
    polygons.retain(|polygon|
        !polygon.buffer(-min_radius, 16).and_then(|remaining| remaining.is_empty())
        .map_err(|e| err = Some(e)).unwrap_or(true)
    );
    if let Some(err) = err {
        return Err(err);
    }
    Ok(polygons)
}
pub fn get_interior_rings<'a>(polygon: &Geometry<'a>) -> geos::GResult<Vec<Geometry<'a>>> {
    (0..polygon.get_num_interior_rings()? as u32)
        .map(|index| polygon.get_interior_ring_n(index).map(|geo| geo.clone()))
        .collect()
}
pub fn get_all_rings_for_polygon<'a>(polygon: &Geometry<'a>) -> geos::GResult<Vec<Geometry<'a>>> {
    let mut rings = get_interior_rings(polygon)?;
    rings.push(Geom::clone(&polygon.get_exterior_ring()?));
    Ok(rings)
}
pub fn get_all_rings<'a>(region: &Geometry<'a>) -> geos::GResult<Vec<Geometry<'a>>> {
    Ok(
        to_polygon_list(region)?.iter()
        .map(get_all_rings_for_polygon)
        .collect::<geos::GResult<Vec<_>>>()?
        .into_iter().flatten().collect()
    )
    //to_geometry_list(&region.boundary()?)

}