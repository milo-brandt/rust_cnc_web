pub struct NamedArcPlane {
    /// Index of the command following G; e.g. "17" or "17.1"
    pub command_index: String,
    pub first_axis: u8,
    pub second_axis: u8,
}
pub struct MachineConfiguration {
    pub axis_characters: Vec<char>,
    pub offset_characters: Vec<char>,
    pub arc_planes: Vec<NamedArcPlane>,
    pub precision: u8,
}
impl MachineConfiguration {
    pub fn standard_3_axis() -> Self {
        Self {
            axis_characters: vec!['X', 'Y', 'Z'],
            offset_characters: vec!['I', 'J', 'K'],
            arc_planes: vec![
                NamedArcPlane { command_index: "17".into(), first_axis: 0, second_axis: 1 },
                NamedArcPlane { command_index: "18".into(), first_axis: 2, second_axis: 0 },
                NamedArcPlane { command_index: "19".into(), first_axis: 1, second_axis: 2 },
            ],
            precision: 3,
        }
    }
    pub fn standard_4_axis() -> Self {
        Self {
            axis_characters: vec!['X', 'Y', 'Z', 'A'],
            offset_characters: vec!['I', 'J', 'K'],
            arc_planes: vec![
                NamedArcPlane { command_index: "17".into(), first_axis: 0, second_axis: 1 },
                NamedArcPlane { command_index: "18".into(), first_axis: 2, second_axis: 0 },
                NamedArcPlane { command_index: "19".into(), first_axis: 1, second_axis: 2 },
            ],
            precision: 3,
        }
    }
}