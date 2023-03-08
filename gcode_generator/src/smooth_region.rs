use geos::{Geom, Geometry};

pub fn allowed_cutter_positions<'a>(to_cut: &Geometry<'a>, to_avoid: &Geometry<'a>, radius: f64, quadsegs: i32) -> geos::GResult<Geometry<'a>> {
    Ok(to_cut.buffer(radius, quadsegs)?.difference(&to_avoid.buffer(radius, quadsegs)?)?)
}
pub fn cuttable_region<'a>(to_cut: &Geometry<'a>, to_avoid: &Geometry<'a>, radius: f64, quadsegs: i32) -> geos::GResult<Geometry<'a>> {
    Ok(allowed_cutter_positions(to_cut, to_avoid, radius, quadsegs)?.buffer(radius, quadsegs)?)
}
pub fn bicuttable_region<'a>(primary: &Geometry<'a>, secondary: &Geometry<'a>, radius: f64, quadsegs: i32) -> geos::GResult<Geometry<'a>> {
    let total = primary.union(secondary)?.buffer(0.01, quadsegs)?.buffer(-0.01, quadsegs)?; // total region with nearby lines joined
    let inner_secondary = cuttable_region(secondary, primary, radius, quadsegs)?.intersection(&total)?;
    let enlarged_primary = cuttable_region(primary, &inner_secondary, radius, quadsegs)?;
    let mating_secondary = cuttable_region(secondary, &enlarged_primary.intersection(&total)?, radius, quadsegs)?;
    /*
        For now: panic if we didn't cover everything. This could happen if the regions were, for instance, two opposite quadrants
        in primary and the rest in secondary; neither cutter would be able to get to the origin in that case. One could develop this
        to be more sophisticated and handle the error by forcing a path to the unreachable point. For now, it shouldn't happen.
    */
    // assert!(enlarged_primary.union(&mating_secondary)?.buffer(0.01, quadsegs)?.contains(&total)?);
    Ok(enlarged_primary)
}
pub fn sequence_cuts<'a>(cuts: Vec<Geometry<'a>>, radius: f64, quadsegs: i32) -> geos::GResult<Vec<Geometry<'a>>> {
    let mut bicuttable_regions = Vec::new();
    // Compute bicuttable regions of each one, avoiding further on ones.
    for i in 0..cuts.len() {
        let mut remaining = Geometry::create_multipolygon(Vec::new())?;
        for j in (i+1)..cuts.len() {
            remaining = remaining.union(&cuts[j])?;
        }
        bicuttable_regions.push(bicuttable_region(&cuts[i], &remaining, radius, quadsegs)?);
    }
    // Then, create co-cuttable regions of each minus prior cuts.
    let mut cocuttable_sequence = Vec::new();
    for i in 0..bicuttable_regions.len() {
        let mut remaining = Clone::clone(&bicuttable_regions[i]);
        for j in 0..i {
            remaining = remaining.difference(&cocuttable_sequence[j])?
        }
        cocuttable_sequence.push(remaining);
    }
    Ok(cocuttable_sequence)
}
pub fn sequence_cuts_non_bleeding<'a>(cuts: Vec<Geometry<'a>>, radius: f64, quadsegs: i32) -> geos::GResult<Vec<Geometry<'a>>> {
    let mut total = Geometry::create_multipolygon(Vec::new())?;
    for cut in &cuts {
        total = total.union(cut)?;
    }
    total = total.buffer(0.01, quadsegs)?.buffer(-0.01, quadsegs)?;
    let mut result = sequence_cuts(cuts, radius, quadsegs)?;
    for item in &mut result {
        *item = item.intersection(&total)?;
    }
    Ok(result)
}