use std::{collections::HashSet, hash::Hash};

use geos::{Geom, Geometry};

use crate::{onion::{OnionTree, onion_tree, onion_graph, OnionGraph, OnionGraphNode}, lines::{join_line_strings, reposition_linear_ring_near_point, line_between_points, last_point_in_string_or_ring, closest_point_on_any_line, ensure_line_string, link_line_strings}, comparable_float::ComparableFloat, collection::get_all_rings};

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

struct CutGenerationState<'a> {
    graph: OnionGraph<'a>,
    existing_cuts: HashSet<usize>,
}
impl<'a> CutGenerationState<'a> {
    fn item_has_children_done(&self, item: &OnionGraphNode<'a>) -> bool {
        for child_index in &item.child_indices {
            if !self.existing_cuts.contains(child_index) {
                return false;
            }
        }
        true
    }
    fn is_candidate(&self, index: usize) -> bool {
        !self.existing_cuts.contains(&index)
        && self.item_has_children_done(&self.graph.items[index])
    }
    fn overall_candidates(&self) -> Vec<usize> {
        (0..self.graph.items.len()).filter(|index| self.is_candidate(*index)).collect()
    }
    fn parent_candidates(&self, last_index: usize) -> Vec<usize> {
        println!("PARENT INDICES: {:?}", self.graph.items[last_index].parent_indices);
        self.graph.items[last_index].parent_indices.iter().copied().filter(|index| self.is_candidate(*index)).collect()
    }
}

struct CutInfo<'a> {
    index: usize,
    joined: bool,
    ring: Geometry<'a>
}
impl<'a> CutInfo<'a> {
    fn optimized(index: usize, joined: bool, ring: &Geometry<'a>, last_point: &Geometry<'a>) -> geos::GResult<Self> {
        println!("GETTING AN OPTIMIZED ONE!");
        Ok(CutInfo {
            index,
            joined,
            ring: reposition_linear_ring_near_point(&ring, &last_point)?
        })
    }
}

// Return a list of line_strings to cut the given shape 
//
// Algorithm: 
// 1. A directed graph containing every ring in the onion-slicing of the region is created;
//    all arrows point from items in one layer to items in the next smaller one. Exterior
//    rings are parents of anything they contain. Interior rings are parents of anything that
//    contains them.
//
//    A node is considered eligible to cut if all of its children have already been cut.
//
// 2. We choose a first cut to make arbitrarily. Then, we iterate by making this cut, looking
//    at our position, and deciding on a next cut as either...:
//      a. ...a parent of the prior cut that is eligible to be cut and on the shortest path to the outside.
//      b. ...any eligible cut that is the closest to our current position.
//    Further rings are repositioned to use whatever is closest to the prior point.
pub fn cut_from_allowable_region<'a>(configuration: &SpiralConfiguration, region: &Geometry<'a>) -> geos::GResult<Vec<Geometry<'a>>> {
    let boundary = get_all_rings(region)?.into_iter().map(|geo| ensure_line_string(&geo)).collect::<geos::GResult<Vec<_>>>()?;
    let mut cut_generation_state = CutGenerationState {
        graph: onion_graph(region, configuration.step_over, configuration.quadsegs, configuration.simplification_tolerance)?,
        existing_cuts: HashSet::new()
    };
    let mut next_cut_info_optional = cut_generation_state.overall_candidates().first().map(|index| {
        CutInfo {
            index: *index,
            joined: false,
            ring: Geom::clone(&cut_generation_state.graph.items[*index].ring)
        }
    });
    let mut paths = Vec::new();
    'path_generate: while let Some(mut next_cut_info) = next_cut_info_optional.take() {
        // Reverse the specified ring if climb milling
        next_cut_info.ring = ensure_line_string(&next_cut_info.ring)?;
        match configuration.milling_mode {
            MillingMode::Normal => (),
            MillingMode::Climb => next_cut_info.ring = next_cut_info.ring.reverse()?,
        }
        // Add in the new cut.
        if next_cut_info.joined {
            let last_path = paths.last_mut().unwrap();
            *last_path = link_line_strings(last_path, &next_cut_info.ring)?;
        } else {
            println!("START OF NEW PATH!!!!");
            println!("START OF NEW PATH!!!!");
            println!("START OF NEW PATH!!!!");
            println!("START OF NEW PATH!!!!");
            println!("START OF NEW PATH!!!!");
            
            paths.push(next_cut_info.ring);
        }
        cut_generation_state.existing_cuts.insert(next_cut_info.index);
        // Then, decide where to go next - first, the parent candidate on the path to the exterior is chosen
        // if it exists. 
        let last_point = last_point_in_string_or_ring(paths.last().unwrap())?;
        let closest_exterior_point = closest_point_on_any_line(&boundary, &last_point)?.ok_or(geos::Error::GenericError("No boundary!".into()))?;
        let line_to_edge = line_between_points(&last_point, &closest_exterior_point)?;
        //paths.insert(0, Geom::clone(&line_to_edge));
        // Loop through parents to see if any intersects the line...

        for parent_candidate_index in cut_generation_state.parent_candidates(next_cut_info.index) {
            let parent_ring = &cut_generation_state.graph.items[parent_candidate_index].ring;
            if parent_ring.distance(&line_to_edge)? < configuration.simplification_tolerance * 0.1 {  //should really use .intersects, but could have rounding issues
                next_cut_info_optional = Some(CutInfo::optimized(
                    parent_candidate_index,
                    true,
                    parent_ring,
                    &last_point
                )?);
                continue 'path_generate;
            }
        }
        // If we didn't find any parents, just look for the closest eligible item.
        let mut err = None;
        let best_candidate = cut_generation_state.overall_candidates().into_iter().min_by_key(|index| {
            ComparableFloat(
                last_point.distance(&cut_generation_state.graph.items[*index].ring)
                .map_err(|e| err = Some(e))
                .unwrap_or(0.0)
            )
        });
        if let Some(err) = err {
            return Err(err)
        }
        if let Some(best_candidate) = best_candidate {
            next_cut_info_optional = Some(CutInfo::optimized(
                best_candidate,
                false,
                &cut_generation_state.graph.items[best_candidate].ring,
                &last_point
            )?);
        }
        println!("GOING ON!");
    }
    println!("CUT: {:?}\nCANDIDATES: {:?}", cut_generation_state.existing_cuts, cut_generation_state.overall_candidates());
    debug_assert!(cut_generation_state.existing_cuts.len() == cut_generation_state.graph.items.len());
    for path in &paths {
        println!("PATH: {}", path.to_wkt()?);
    }
    Ok(paths)
}