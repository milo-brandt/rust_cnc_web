use crate::{coordinates::PartialPosition, config::MachineConfiguration, parse::parse_line, gcode::{CommandContent, HelicalMove, LinearMove, ProbeMove}};

#[derive(Default, Debug)]
pub struct EstimatedExtent {
    pub bounds: Vec<Option<(f64, f64)>>
}
impl EstimatedExtent {
    fn extend_to(&mut self, position: &PartialPosition) {
        if self.bounds.len() < position.0.len() {
            self.bounds.resize_with(position.0.len(), || None);
        }
        for (bound, point) in self.bounds.iter_mut().zip(position.0.iter()) {
            if let Some(point) = point {
                match bound {
                    Some((min, max)) => {
                        *min = f64::min(*min, *point);
                        *max = f64::max(*max, *point);
                    },
                    None => *bound = Some((*point, *point)),
                }
            }
        }
    }
}

pub fn estimate_extent(
    config: &MachineConfiguration,
    input: &str,
) -> Result<EstimatedExtent, usize> {
    let mut extent = EstimatedExtent::default();
    for (index, line) in input.lines().enumerate() {
        if line.trim_start().starts_with("(") || line.trim_start().starts_with("M") || line.trim() == "" {
            continue
        }
        let line = match parse_line(config, line) {
            Some(parsed_line) => parsed_line,
            None => return Err(index),
        };
        let target = match line.command {
            Some(CommandContent::HelicalMove(HelicalMove { target, .. })) => Some(target),
            Some(CommandContent::LinearMove(LinearMove(target))) => Some(target),
            Some(CommandContent::ProbeMove(ProbeMove(_, target))) => Some(target),
            None => None,
        };
        if let Some(target) = target {
            extent.extend_to(&target);
        }
    }
    Ok(extent)
}
