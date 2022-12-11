
use std::vec;

use geos::{Geom, Geometry, CoordSeq};

// Represents the result of cutting: either two pieces or just one.
pub enum CutLine<'a> {
    TwoPart(Geometry<'a>, Geometry<'a>),
    OnePart(Geometry<'a>)
}

impl <'a, 'b> IntoIterator for &'a CutLine<'b> {
    type Item = &'a Geometry<'b>;

    type IntoIter = vec::IntoIter<&'a Geometry<'b>>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            CutLine::TwoPart(first, second) => vec![first, second].into_iter(),
            CutLine::OnePart(first) => vec![first].into_iter(),
        }
    }
}

impl <'a> IntoIterator for CutLine<'a> {
    type Item = Geometry<'a>;

    type IntoIter = vec::IntoIter<Geometry<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            CutLine::TwoPart(first, second) => vec![first, second].into_iter(),
            CutLine::OnePart(first) => vec![first].into_iter(),
        }
    }
}

/*
    if distance <= 0.0 or distance >= line.length:
        return [LineString(line)]
    coords = list(line.coords)
    for i, p in enumerate(coords):
        pd = line.project(Point(p))
        if pd == distance:
            return [
                LineString(coords[:i+1]),
                LineString(coords[i:])]
        if pd > distance:
            cp = line.interpolate(distance)
            return [
                LineString(coords[:i] + [(cp.x, cp.y)]),
                LineString([(cp.x, cp.y)] + coords[i:])]
    return [LineString(line)]

*/
pub fn coord_seq_to_vec(coord_seq: &CoordSeq) -> geos::GResult<Vec<[f64; 2]>> {
    (0..coord_seq.number_of_lines()?).map(|index| -> geos::GResult<[f64; 2]> { 
        Ok([coord_seq.get_x(index)?, coord_seq.get_y(index)?])
    }).collect()
}
pub fn coordinates_to_point<'a>(coordinates: &[f64; 2]) -> geos::GResult<Geometry<'a>> {
    Geometry::create_point(
        CoordSeq::new_from_vec(&[coordinates])?
    )
}

pub fn cut_line_at<'a>(line_string: &Geometry<'a>, distance: f64) -> geos::GResult<CutLine<'a>> {
    if distance <= 0.0 {
        return Ok(CutLine::OnePart(Geom::clone(line_string)));
    }
    let mut coords = coord_seq_to_vec(&line_string.get_coord_seq()?)?;
    for (index, coord) in coords.iter().enumerate() {
        let point = Geometry::create_point(CoordSeq::new_from_vec(&[coord])?)?;
        let current_distance = line_string.project(&point)?;
        if index < coords.len() - 1 && current_distance == distance { // cut is right at this vertex!
            return Ok(CutLine::TwoPart(
                Geometry::create_line_string(CoordSeq::new_from_vec(&coords[..index+1])?)?,
                Geometry::create_line_string(CoordSeq::new_from_vec(&coords[index..])?)?,
            ))
        }
        if current_distance > distance {
            let cut_point = line_string.interpolate(distance)?;
            // Insert the new point into the given index. Could do this more efficiently, but don't really need to.
            coords.insert(index, [cut_point.get_x()?, cut_point.get_y()?]);
            return Ok(CutLine::TwoPart(
                Geometry::create_line_string(CoordSeq::new_from_vec(&coords[..index+1])?)?,
                Geometry::create_line_string(CoordSeq::new_from_vec(&coords[index..])?)?,
            ));
        }
    }
    // Distance is greater than line length - only one part!
    return Ok(CutLine::OnePart(Geom::clone(line_string)));
}
pub fn cut_line_near_point<'a>(line_string: &Geometry<'a>, point: &Geometry<'a>) -> geos::GResult<CutLine<'a>> {
    cut_line_at(line_string, line_string.project(point)?)
}
pub fn cut_line_near_coordinates<'a>(line_string: &Geometry<'a>, coordinates: [f64; 2]) -> geos::GResult<CutLine<'a>> {
    cut_line_near_point(line_string, &coordinates_to_point(&coordinates)?)
}


pub fn reposition_linear_ring_to<'a>(linear_ring: &Geometry<'a>, distance: f64) -> geos::GResult<Geometry<'a>> {
    if !linear_ring.is_closed()? {
        return Err(geos::Error::GenericError("Cannot reposition an open line string".into()));
    }
    match cut_line_at(linear_ring, distance)? {
        CutLine::TwoPart(first, second) => join_line_strings(&second, &first),
        CutLine::OnePart(only_part) => Ok(only_part),
    }
}
pub fn reposition_linear_ring_near_point<'a>(line_string: &Geometry<'a>, point: &Geometry<'a>) -> geos::GResult<Geometry<'a>> {
    reposition_linear_ring_to(line_string, line_string.project(point)?)
}
pub fn reposition_linear_ring_near_coordinates<'a>(line_string: &Geometry<'a>, coordinates: [f64; 2]) -> geos::GResult<Geometry<'a>> {
    reposition_linear_ring_near_point(line_string, &coordinates_to_point(&coordinates)?)
}

// Given two line strings, the first ending where the second begins, fuse them into one line string.
pub fn join_line_strings<'a>(first: &Geometry<'a>, second: &Geometry<'a>) -> geos::GResult<Geometry<'a>> {
    if !first.get_end_point()?.equals(&second.get_start_point()?)? {
        return Err(geos::Error::GenericError("Cannot append line strings with differing start/end points".into()))
    }
    let mut coords = coord_seq_to_vec(&first.get_coord_seq()?)?;
    let second_coords = coord_seq_to_vec(&second.get_coord_seq()?)?;
    coords.extend(second_coords[1..].iter()); // exclude first point of second - same as last of first.
    Ok(Geometry::create_line_string(CoordSeq::new_from_vec(&coords)?)?)
}

// Given two line strings, create a new one traversing the first path, then moving to the second one.
pub fn link_line_strings<'a>(first: &Geometry<'a>, second: &Geometry<'a>) -> geos::GResult<Geometry<'a>> {
    let mut coords = coord_seq_to_vec(&first.get_coord_seq()?)?;
    let mut second_coords = coord_seq_to_vec(&second.get_coord_seq()?)?;
    coords.append(&mut second_coords);
    Ok(Geometry::create_line_string(CoordSeq::new_from_vec(&coords)?)?)
}


