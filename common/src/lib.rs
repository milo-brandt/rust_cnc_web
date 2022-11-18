use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct MachineStatus {
    pub status: String,
    pub position: Vec<f64>,
}
