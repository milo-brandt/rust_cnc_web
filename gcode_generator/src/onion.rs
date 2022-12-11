use std::mem;

use geos::{Geom, Geometry};

use crate::collection::to_geometry_list;

pub fn onion_layers<'a>(geometry: &Geometry<'a>, offset_size: f64, quadsegs: i32, simplification_tolerance: f64) -> geos::GResult<Vec<Geometry<'a>>> {
    let mut layers = Vec::new();
    let mut offset = 0.0;
    loop {
        let mut next_geometry = geometry.buffer(-offset, quadsegs)?.simplify(simplification_tolerance)?;
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