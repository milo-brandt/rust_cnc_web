use geos::{Geom, Geometry};

use crate::{onion::{OnionTree, onion_tree}, lines::{join_line_strings, reposition_linear_ring_near_point}};

#[derive(Debug, Copy, Clone)]
pub enum MillingMode {
    Normal, // Traverse clockwise, keeping the cutting edge on the left of the clockwise-spinning bit
    Climb,
}

#[derive(Debug, Copy, Clone)]
pub struct SpiralConfiguration {
    pub step_over: f64,
    pub milling_mode: MillingMode,
    pub simplification_tolerance: f64, // How much the path may deviate from the theoretical amount (to decrease number of segments)
    pub quadsegs: i32, // How many segments in each quarter of a circle.
}

/*
pub fn cut_from_onion_tree<'a>(mode: MillingMode, tree: &OnionTree<'a>) -> geos::GResult<Vec<Geometry<'a>>> {
    let mut child_cuts = tree.children.iter()
        .map(|subtree| cut_from_onion_tree(mode, subtree))
        .collect::<geos::GResult<Vec<_>>>()?.into_iter().flatten().collect::<Vec<_>>();
    if let Some(last_cut) = child_cuts.last_mut() {
        let last_point = last_cut.get_end_point()?;
        *last_cut = join_line_strings(
            last_cut,
            reposition_linear_ring_near_point(line_string, last_point)
        );
    }
    todo!()
}
*/

// Return a list of line_strings to cut the given shape 
pub fn cut_from_allowable_region<'a>(configuration: &SpiralConfiguration, region: &Geometry<'a>) -> geos::GResult<Vec<Geometry<'a>>> {
    let tree = onion_tree(region, configuration.step_over, configuration.quadsegs, configuration.simplification_tolerance);


    todo!()
}