use config::MachineConfiguration;
use output::MachineFormatter;
use parse::parse_line;
use simple::SimpleTransform;

pub mod probe;
pub mod gcode;
/// A module representing transforms by the symmetries of a (hyper-)cube.
pub mod simple;
/// A module with utilities for creating transforms that operate based on input points.
pub mod pointwise;
pub mod coordinates;
pub mod config;
pub mod output;
pub mod parse;
/// Utilities for transforming GCode files into series of line segments.
pub mod lines;
pub mod tag;

// pub fn transform_gcode_file(
//     config: &MachineConfiguration,
//     transform: &SimpleTransform,
//     input: &str,
// ) -> Result<String, usize> {
//     let mut transformer = CommandTransformer::new(
//         transform,
//         config.arc_planes.iter().map(|x| ArcPlane(x.first_axis, x.second_axis)).collect()
//     );
//     let mut result = input.lines().enumerate().map(|(index, line)|
//         if line.trim_start().starts_with("(") || line.trim_start().starts_with("M") {
//             Ok(format!("{}\n", line))
//         } else {
//             parse_line(config, line)
//             .and_then(|line|
//                 transformer.transform(&line).ok()
//             )
//             .map(|line|
//                 format!("{}\n", MachineFormatter(config, &line))
//             ).ok_or_else(
//                 || index
//             )
//         }
//     ).collect::<Result<String, usize>>()?;
//     // Trim ending whitespace to avoid repeated transformations adding lots of empty lines
//     // at the end.
//     loop {
//         match result.pop() {
//             Some(c) if c.is_whitespace() => (),
//             Some(c) => {
//                 result.push(c);
//                 result.push('\n');
//                 return Ok(result)
//             }
//             None => return Ok(result)
//         }
//     }
// }
// #[cfg(test)]
// mod tests {
//     use crate::{simple::{SignedIndex, Sign::Positive}, coordinates::Offset};

//     use super::*;

//     #[test]
//     fn it_works() {
//         let config = &MachineConfiguration::standard_4_axis();
//         let lines = r"
//             G90 G17 G21
//             G0 X0 Y0 Z0 A4
//             G2 I10 X20
//         ";
//         eprintln!("{}", transform_gcode_file(
//             config,
//             &SimpleTransform {
//                 permutation: vec![
//                     SignedIndex(Positive, 2),
//                     SignedIndex(Positive, 1),
//                     SignedIndex(Positive, 0),
//                     SignedIndex(Positive, 3)
//                 ],
//                 offset: Offset(vec![1.0, 2.0, 3.0, 4.0])
//             },
//             lines
//         ).unwrap())
//     }
// }
