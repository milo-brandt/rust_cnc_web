use std::mem;

use geos::{Geom, Geometry};

use crate::{collection::{to_geometry_list, get_interior_rings}, lines::linear_ring_to_polygon};

pub fn onion_layers<'a>(geometry: &Geometry<'a>, offset_size: f64, quadsegs: i32, simplification_tolerance: f64) -> geos::GResult<Vec<Geometry<'a>>> {
    let mut layers = Vec::new();
    let mut offset = 0.0;
    loop {
        let mut next_geometry = geometry.buffer(-offset, quadsegs)?.topology_preserve_simplify(simplification_tolerance)?;
        next_geometry.normalize()?;
        if next_geometry.is_empty()? {
            return Ok(layers);
        } else {
            layers.push(next_geometry);
        }
        offset += offset_size;
    }
}

pub struct OnionTree<'a> {
    pub polygon: Geometry<'a>,
    pub children: Vec<OnionTree<'a>>,    
}

// The current state of the onion tree building; contains anything not yet sorted.
struct UntamedOnionLayers<'a> {
    layers: Vec<Vec<Geometry<'a>>>,
}
impl<'a> UntamedOnionLayers<'a> {
    fn extract_child_trees(&mut self, parent: &Geometry<'a>, layer_index: usize) -> geos::GResult<Vec<OnionTree<'a>>> {
        if layer_index >= self.layers.len() {
            return Ok(Vec::new());
        }
        let layer_ref = &mut self.layers[layer_index];
        //let layer = mem::take(layer_ref);
        let mut failure_reason = None;
        let (children, others) = layer_ref.drain(..).partition(|candidate| {
            parent.contains(candidate).map_err(|err| failure_reason = Some(err)).unwrap_or(false)
        });
        if let Some(error) = failure_reason {
            return Err(error);
        }
        *layer_ref = others;
        return children.into_iter().map(|child| self.extract_tree(child, layer_index)).collect()
    }
    fn extract_tree(&mut self, parent: Geometry<'a>, parent_layer_index: usize) -> geos::GResult<OnionTree<'a>> {
        let children = self.extract_child_trees(&parent, parent_layer_index + 1)?;
        Ok(OnionTree { polygon: parent, children })
    }
    fn to_onion_trees(mut self) -> geos::GResult<Vec<OnionTree<'a>>> {
        if self.layers.is_empty() {
            return Ok(Vec::new());
        }
        let result = mem::take(&mut self.layers[0]).into_iter().map(|top_level| self.extract_tree(top_level, 0)).collect();
        for layer in &self.layers {
            debug_assert!(layer.is_empty());
        }
        return result
    }
}

pub fn onion_tree<'a>(geometry: &Geometry<'a>, offset_size: f64, quadsegs: i32, simplification_tolerance: f64) -> geos::GResult<Vec<OnionTree<'a>>> {
    let layers = onion_layers(geometry, offset_size, quadsegs, simplification_tolerance)?
        .into_iter().map(|layer| {
            to_geometry_list(&layer)
        }).collect::<geos::GResult<Vec<_>>>()?;
    UntamedOnionLayers{ layers }.to_onion_trees()
}



/*
    A representation of just the rings in the onion ordered such that...
    1. Exterior rings are parents of everything in the layer beneath contained within.
    2. Interior rings are parents of everything in the layer beneath containing them.
*/

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RingKind {
    Exterior,
    Interior
}

pub struct OnionGraphNode<'a> {
    pub kind: RingKind,
    pub ring: Geometry<'a>,
    pub parent_indices: Vec<usize>,
    pub child_indices: Vec<usize>,
}
pub struct OnionGraph<'a> {
    pub items: Vec<OnionGraphNode<'a>>
}

impl<'a> OnionGraphNode<'a> {
    fn from_kind_and_ring(kind: RingKind, ring: Geometry<'a>) -> Self {
        OnionGraphNode {
            kind,
            ring,
            parent_indices: Vec::new(),
            child_indices: Vec::new(),
        }
    }
}

impl<'a> OnionGraph<'a> {
    fn add_parent_child_relationship(&mut self, parent_index: usize, child_index: usize) {
        self.items[parent_index].child_indices.push(child_index);
        self.items[child_index].parent_indices.push(parent_index);
    }
    fn add_from_polygon(&mut self, polygon: &Geometry<'a>) -> geos::GResult<()> {
        self.items.push(
            OnionGraphNode::from_kind_and_ring(RingKind::Exterior, polygon.get_exterior_ring()?.clone())
        );
        for interior_ring in get_interior_rings(polygon)? {
            self.items.push(
                OnionGraphNode::from_kind_and_ring(RingKind::Interior, interior_ring)
            );
        }
        Ok(())
    }
}

/*
    Two edges are related if the outer layer minus the inner one has a path between the rings in question.
*/
pub fn onion_graph<'a>(geometry: &Geometry<'a>, offset_size: f64, quadsegs: i32, simplification_tolerance: f64) -> geos::GResult<OnionGraph<'a>> {
    let mut result = OnionGraph { items: Vec::new() };
    let mut last_layer_start = 0;
    let mut last_layer_end = 0;
    let mut parent_child_relationships = Vec::new();
    let mut inherit_children_from = Vec::new(); //pairs (parent, inheritor) to copy relations from!
    for layer in onion_layers(geometry, offset_size, quadsegs, simplification_tolerance)? {
        // Find all the rings in this layer, and put them in.
        for polygon in to_geometry_list(&layer)? {
            result.add_from_polygon(&polygon)?;
        }
        // Iterate through all pairs of (something from prior layer, something in this layer)
        let layer_end = result.items.len();
        for parent_index in last_layer_start..last_layer_end {
            let parent_item = &result.items[parent_index];
            let mut has_child = false;
            for child_index in last_layer_end..layer_end {
                let child_item = &result.items[child_index];
                // Check whether the parent relationship holds
                // * if parent is exterior: does it contain the child?
                // * if parent is interior: does the child contain it?
                let has_parent_child_relation = match parent_item.kind {
                    RingKind::Exterior => linear_ring_to_polygon(&parent_item.ring)?.contains(&child_item.ring)?,
                    RingKind::Interior => linear_ring_to_polygon(&child_item.ring)?.contains(&parent_item.ring)?,
                } && parent_item.kind == child_item.kind;
                if has_parent_child_relation {
                    parent_child_relationships.push((parent_index, child_index));
                    has_child = true;
                }
            }
            // If this is an interior ring not within the last layer, copy the children of the
            // exterior ring which contains it.
            if !has_child && parent_item.kind == RingKind::Interior {
                // Can just iterate backwards until we find it...
                let mut last_parent = parent_index;
                loop {
                    last_parent -= 1;
                    if result.items[last_parent].kind == RingKind::Exterior {
                        inherit_children_from.push((last_parent, parent_index));
                        break;
                    }
                }
            }
        }
        last_layer_start = last_layer_end;
        last_layer_end = layer_end;
    }
    // This is all very hacky to get around lifetimes :/
    for (parent_index, child_index) in parent_child_relationships {
        result.add_parent_child_relationship(parent_index, child_index);
    }
    for (parent_index, inheritor_index) in inherit_children_from {
        for child_index in result.items[parent_index].child_indices.clone() {
            result.add_parent_child_relationship(inheritor_index, child_index);
        }
    }
    Ok(result)
}