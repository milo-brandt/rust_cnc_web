use config::MachineConfiguration;
use gcode::{CommandTransformer, ArcPlane};
use output::MachineFormatter;
use parse::parse_line;
use transform::SimpleTransform;

pub mod probe;
pub mod gcode;
pub mod transform;
pub mod coordinates;
pub mod config;
pub mod output;
pub mod parse;

pub fn transform_gcode_file(
    config: &MachineConfiguration,
    transform: &SimpleTransform,
    input: &str,
) -> Option<String> {
    let mut transformer = CommandTransformer::new(
        transform,
        config.arc_planes.iter().map(|x| ArcPlane(x.first_axis, x.second_axis)).collect()
    );
    input.lines().map(|line| 
        parse_line(config, line)
        .and_then(|line|
            transformer.transform(&line).ok()
        )
        .map(|line|
            format!("{}\n", MachineFormatter(config, &line))
        )
    ).collect::<Option<String>>()
}
#[cfg(test)]
mod tests {
    use crate::{transform::{SignedIndex, Sign::Positive, Sign::Negative}, coordinates::Offset};

    use super::*;

    #[test]
    fn it_works() {
        let config = &MachineConfiguration::standard_4_axis();
        let lines = r"
            G90 G17 G21
            G0 X0 Y0 Z0 A4
            G2 I10 X20
        ";
        eprintln!("{}", transform_gcode_file(
            config,
            &SimpleTransform {
                permutation: vec![
                    SignedIndex(Positive, 2),
                    SignedIndex(Positive, 1),
                    SignedIndex(Positive, 0),
                    SignedIndex(Positive, 3)
                ],
                offset: Offset(vec![1.0, 2.0, 3.0, 4.0])
            },
            lines
        ).unwrap())
    }
}
