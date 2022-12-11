use geos::{Geom, Geometry};

use crate::{spiral_path, spiral_path::{SpiralConfiguration, MillingMode}, collection::{to_geometry_list, to_polygon_list, to_polygon_list_removing_small}};

pub struct CuttingStep {
    pub tool_radius: f64,
    pub step_over: f64,
    pub safety_margin: f64,
    pub simplification_tolerance: f64,
    pub quadsegs: i32,
    pub milling_mode: MillingMode,
    pub profile_pass: bool,
}
pub struct ForegroundCutInfo<'b, 'a> {
    pub required_region: &'b Geometry<'a>,
    pub allowed_region: &'b Geometry<'a>,
    pub cut_region: Geometry<'a>,
}
pub struct CutOutput<'a> {
    pub steps: Vec<Vec<Geometry<'a>>>,
    pub total_cut: Geometry<'a>,
}

impl<'b, 'a> ForegroundCutInfo<'b, 'a> {
    pub fn add_step(&mut self, step: &CuttingStep) -> geos::GResult<Vec<Geometry<'a>>> {
        // Generate profile pass if desired.
        let mut profile_pass = if step.profile_pass {
            // Note: Don't need to filter by allowable_positions here because we assume required_region is a subset of allowed_region.
            let productive_boundary = self.required_region
                .buffer(-step.tool_radius - step.safety_margin, step.quadsegs)?
                .boundary()?
                .difference(&self.cut_region.buffer(-step.tool_radius - step.safety_margin, step.quadsegs)?)?
                .simplify(step.simplification_tolerance)?;
            self.cut_region = self.cut_region.union(&productive_boundary.buffer(step.tool_radius, step.quadsegs)?)?;
            to_geometry_list(&productive_boundary)? // TODO: Optimize by going to closest next part each time!
        } else {
            Vec::new()
        };
        // Generate clearing pass.
        let allowable_positions = self.allowed_region.buffer(-step.tool_radius - step.safety_margin, step.quadsegs)?;
        let mut remaining = self.required_region.difference(&self.cut_region)?;
        if step.profile_pass {
            // if doing a profile pass, kick out anything near the boundary... they should be assumed to have been hit already!
            let near_boundary = self.required_region.boundary()?.buffer(2.0 * step.tool_radius + step.safety_margin + step.simplification_tolerance, step.quadsegs)?;
            remaining = remaining.difference(&near_boundary)?;
        };
        let remaining = Geometry::create_multipolygon(
            to_polygon_list_removing_small(
                &remaining,
                 step.simplification_tolerance, // not really a good way to do this...
            )?
        )?;

        let mut productive_positions = remaining.buffer(step.tool_radius, step.quadsegs)?;  // plus subtract out profiling here?
        if step.profile_pass {
            // for a profile pass, also don't allow things within the stepover of the profile
            productive_positions = productive_positions.difference(
                &self.required_region.boundary()?.buffer(step.tool_radius + step.safety_margin + step.step_over, step.quadsegs)?
            )?;
        }
        let positions_to_cut = allowable_positions.intersection(&productive_positions)?;
        // Look for and remove any tiny polygons...
        /*let mut polygons_to_cut = to_polygon_list(&positions_to_cut)?;
        let mut err = None;
        polygons_to_cut.retain(|polygon| {
            polygon.buffer(-step.simplification_tolerance, step.quadsegs).and_then(|remaining| remaining.is_empty())
            .map_err(|e| err = Some(e)).unwrap_or(false)
        });
        if let Some(err) = err {
            return Err(err);
        }
        let positions_to_cut = Geometry::create_multipolygon(polygons_to_cut)?;*/

        self.cut_region = self.cut_region.union(&positions_to_cut.buffer(step.tool_radius, step.quadsegs)?)?;
        let mut clearing_pass = spiral_path::cut_from_allowable_region(
            &SpiralConfiguration {
                step_over: step.step_over,
                milling_mode: step.milling_mode,
                simplification_tolerance: step.simplification_tolerance,
                quadsegs: step.quadsegs,
            },
            &positions_to_cut,
        )?;
        clearing_pass.append(&mut profile_pass);
        Ok(clearing_pass)
    }
}