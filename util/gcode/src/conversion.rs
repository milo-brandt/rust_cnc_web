// use itertools::Itertools;

// use crate::gcode_motion::{Position, Motion};

// #[derive(Clone, Debug, Copy, PartialEq, Eq)]
// pub enum ArcPlane {
//     XY,
//     XZ,
//     YZ,
// }
// #[derive(Clone, Debug, Copy, PartialEq, Eq)]
// pub enum ArcOrientation {
//     Clockwise,
//     Counterclockiwse,
// }
// #[derive(Clone, Debug, Copy, PartialEq, Eq)]
// pub struct ArcState {
//     plane: ArcPlane,
//     orientation: ArcOrientation,
// }
// #[derive(Clone, Debug, Copy, PartialEq, Eq)]
// pub struct PartialModalState {
//     pub arc_plane: Option<ArcPlane>,
//     pub arc_orientation: Option<ArcOrientation>,
// }
// #[derive(Clone, Debug)]
// pub struct PartialMachineState {
//     pub modal_state: PartialModalState,
//     pub position: Position,
// }
// pub struct Configuration {
//     pub names: Vec<char>,
//     pub offset_names: Vec<char>
// }
// pub fn format(
//     configuration: &Configuration,
//     mut machine_state: PartialMachineState,
//     motion: Motion,
// ) -> (PartialMachineState, String) {
//     match motion {
//         Motion::LinearMotion(motion) => {
//             let axis_words = motion.target.0.iter().enumerate().filter_map(|(index, value)| {
//                 if machine_state.position.0[index] == *value {
//                     None
//                 } else {
//                     // TODO: Allow customizing float formatting.
//                     Some(format!("{}{}", configuration.names[index], value))
//                 }
//             }).join(" "); // TODO: what if it's empty?
//             let line = if axis_words.len() > 0 {
//                 format!("G1 {}\n", axis_words)
//             } else {
//                 String::new()
//             };
//             (
//                 PartialMachineState {
//                     position: motion.target,
//                     ..machine_state
//                 },
//                 line
//             )
//         },
//         Motion::HelicalMotion(motion) => {
//             let axis_words = motion.target.0.iter().enumerate().filter_map(|(index, value)| {
//                 if machine_state.position.0[index] == *value {
//                     None
//                 } else {
//                     // TODO: Allow customizing float formatting.
//                     Some(format!("{}{}", configuration.names[index], value))
//                 }
//             }).join(" "); // TODO: what if it's empty?
//             let offset_words = format!(
//                 "{}{} {}{}",
//                 configuration.offset_names[motion.principal_axis as usize],
//                 motion.principal_center - machine_state.position.0[motion.principal_axis as usize],
//                 configuration.offset_names[motion.secondary_axis as usize],
//                 motion.secondary_center - machine_state.position.0[motion.secondary_axis as usize],
//             );
//             // TODO: Work out modals, G2 vs. G3
//             let line = format!("G2 {} {}", axis_words, offset_words);
//             (
//                 PartialMachineState {
//                     position: motion.target,
//                     ..machine_state
//                 },
//                 line
//             )
//         },
//     }
// }