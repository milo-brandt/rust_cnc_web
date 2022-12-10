use std::mem;

use geos::{Geom, Geometry, CoordDimensions, CoordSeq};

#[derive(Debug, Copy, Clone)]
pub enum MillingMode {
    Normal,
    Climb,
}

#[derive(Debug, Copy, Clone)]
pub struct SpiralConfiguration {
    pub step_over: f64,
    pub prerequisite_distance: f64, // When an inner ring is at least this close to an outer one, require it be routed first.
    pub non_reposition_distance: f64, // How long we can transition from one point to the next before we should raise the spindle & do a rapid
    pub milling_mode: MillingMode,
    pub simplification_tolerance: f64, // How much the path may deviate from the theoretical amount (to decrease number of segments)
    pub quadsegs: i32, // How many segments in each quarter of a circle.
}

#[derive(Copy, Clone)]
pub struct RegionSpecification<'a, 'b> {
    pub region: &'a Geometry<'b>,
    pub excluded: &'a Geometry<'b>, 
}

pub struct Spiral {
    paths: Vec<Vec<[f64; 2]>>
}
impl Spiral {
    fn push_line_strip(&mut self, mut line_strip: Vec<[f64; 2]>, with_prior: bool) -> geos::GResult<()> {
        if with_prior && !self.paths.is_empty() {
            let last_element = self.paths.last_mut().unwrap();  // safe because non-empty!
            last_element.append(&mut line_strip);
        } else {
            self.paths.push(line_strip);
        }
        Ok(())
    }
    fn get_last_position(&self) -> Option<[f64; 2]> {
        match self.paths.last() {
            Some(last) => last.last().map(|v| *v),
            None => None,
        }
    }
}

/*
    Utilities
*/
pub fn onion_layers_until<'a>(geometry: &Geometry<'a>, minimum: &Geometry<'a>, offset_size: f64, quadsegs: i32) -> geos::GResult<Vec<Geometry<'a>>> {
    let mut layers = Vec::new();
    let mut offset = 0.0;
    loop {
        let next_geometry = geometry.buffer(-offset, quadsegs)?;
        if next_geometry.is_empty()? || minimum.contains(&next_geometry)? {
            return Ok(layers);
        } else {
            layers.push(next_geometry);
        }
        offset += offset_size;
    }
}
pub fn onion_layers<'a>(geometry: &Geometry<'a>, offset_size: f64, quadsegs: i32) -> geos::GResult<Vec<Geometry<'a>>> {
    onion_layers_until(geometry, &Geometry::create_empty_polygon()?, offset_size, quadsegs)
}

pub fn to_geometry_list<'a>(geometry: &Geometry<'a>) -> geos::GResult<Vec<Geometry<'a>>> {
    let length = geometry.get_num_geometries()?;
    let mut result = Vec::with_capacity(length);
    for index in 0..length {
        result.push(geometry.get_geometry_n(index)?.clone());
    }
    Ok(result)
}

// A number of optimizations could be had here - dependents could be flattened and items & dependency_count could be pre-allocated. Probably doesn't matter.
struct GraphEntry<'a> {
    geometry: Geometry<'a>,
    path: Vec<[f64; 2]>,
    dependency_count: usize, // how many things this cut is dependent upon
    layer_index: usize,
    dependencies_included: usize,
    dependents: Vec<usize>,
}
struct DependencyGraph<'a> {
    items: Vec<GraphEntry<'a>>,
}

fn line_strip_to_coordinates(geometry: &Geometry) -> geos::GResult<Vec<[f64; 2]>> {
    let coord_seq = geometry.get_coord_seq()?;
    assert!(coord_seq.dimensions()? == CoordDimensions::TwoD);
    (0..coord_seq.number_of_lines()?).map(|index| -> geos::GResult<[f64;2]> {
        Ok([coord_seq.get_x(index)?, coord_seq.get_y(index)?])
    }).collect()
}

fn distance_squared(a: &[f64; 2], b: &[f64; 2]) -> f64 {
    (a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1])
}
pub struct ComparableFloat(pub f64);

impl PartialEq for ComparableFloat {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl PartialOrd for ComparableFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Eq for ComparableFloat {}

impl Ord for ComparableFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.partial_cmp(&other.0).unwrap()
    }
}


impl SpiralConfiguration {
    pub fn convert_to_path<'a: 'b, 'b>(&self, specification: RegionSpecification<'a, 'b>) -> geos::GResult<Spiral> {
        /*
            First, calculate the onion-ing sequence with simplification
        */
        let mut layers = onion_layers_until(specification.region, specification.excluded, self.step_over, self.quadsegs)?;
        layers.reverse();
        // Normalize & simplify all involved geometry
        for layer in &mut layers {
            layer.normalize()?;
            *layer = layer.simplify(self.simplification_tolerance)?;
        }
        // Cut out the exclusion region and generate a list of the remaining LineStrings and LinearRings.
        let boundaries_by_layer = 
            layers.iter().map(|geometry| {
                geometry.boundary()
                .and_then(|boundary| boundary.difference(specification.excluded))
                .and_then(|reduced_boundary| to_geometry_list(&reduced_boundary))
            })
            .collect::<geos::GResult<Vec<_>>>()?;
        /*
            Then, build a graph where we keep track of what in each layer depends upon what in the prior layer.
        */
        let mut graph = DependencyGraph {
            items: Vec::new(),
        };
        let mut last_layer_start = 0; // pointers for where the last layer is stored...
        let mut last_layer_end = 0;
        
        for (layer_index, boundaries) in boundaries_by_layer.into_iter().enumerate() { // Loop through each layer...
            for item in boundaries.into_iter() { // and each item in each layer...
                let my_index = graph.items.len();
                let mut dependency_count = 0;
                for last_layer_index in last_layer_start..last_layer_end { // and each item in the last layer...
                    if item.distance(&graph.items[last_layer_index].geometry)? < self.prerequisite_distance {
                        // Update dependencies if we find one!
                        graph.items[last_layer_index].dependents.push(my_index);
                        dependency_count += 1;
                    }
                }
                // Put the new item into our graph.
                let path = line_strip_to_coordinates(&item)?;
                graph.items.push(GraphEntry{
                    geometry: item,
                    path,
                    dependency_count,
                    layer_index,
                    dependencies_included: 0,
                    dependents: Vec::new(),
                });
            }
            last_layer_start = last_layer_end;
            last_layer_end = graph.items.len();
        }
        /*
            Now that we have the layers, keep track of which elements are eligible to move to.

            Use a greedy algorithm: from each current position, find the closest next starting location and move there.
            If the distance to move is less than self.non_rapid_distance AND is fully contained within the layer of the
            outer path, this can be part of the current movement. Otherwise, use a repositioning motion to get over.

            TODO: Find the closest point on rings and consider that as a starting point!
        */
        let mut items_ready = (0..graph.items.len()).filter(|index| graph.items[*index].dependency_count == 0).collect::<Vec<_>>();
        let mut result = Spiral {
            paths: Vec::new(),
        };

        // Funny loop: Start with an arbitrary item, then loop while more items are found!
        let mut with_prior = false;
        let mut next_item = items_ready.pop();

        while let Some(next_index) = next_item {
            // Record our end position...
            let last_position = *graph.items[next_index].path.last().unwrap();
            // Then add it to the result
            result.push_line_strip(mem::take(&mut graph.items[next_index].path), with_prior)?;
            // Update dependencies to see what's eligible to cut next.
            for dependent_index in mem::take(&mut graph.items[next_index].dependents) {
                graph.items[dependent_index].dependencies_included += 1;
                if graph.items[dependent_index].dependencies_included == graph.items[dependent_index].dependency_count {
                    items_ready.push(dependent_index);
                }
            }
            // Search for the nearest starting point.
            next_item = items_ready.iter().copied().max_by_key(|candidate| {
                let starting_position = graph.items[*candidate].path[0];
                ComparableFloat(distance_squared(&starting_position, &last_position))
            });
            with_prior = false;
            if let Some(next_index) = &next_item {
                let next_item = &graph.items[*next_index];
                let starting_position = next_item.path[0];
                if distance_squared(&last_position, &starting_position) < self.non_reposition_distance * self.non_reposition_distance {
                    // Check that it stays within the needed bounds
                    let required_layer = &layers[next_item.layer_index];
                    let coords = CoordSeq::new_from_vec(&[
                        last_position,
                        [  // slightly shortened to avoid any sort of weird rounding issues.
                            last_position[0] * 0.01 + starting_position[0] * 0.99,
                            last_position[1] * 0.01 + starting_position[1] * 0.99,
                        ]
                    ])?;
                    let line = Geometry::create_line_string(coords)?;
                    with_prior = required_layer.contains(&line)?;
                }
                // Pop the best index off of the vector.
                items_ready.retain(|item| item != next_index);
            }
        }
        Ok(result)
    }
}