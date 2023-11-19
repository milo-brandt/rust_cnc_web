use std::fs::read_to_string;

use clap::{Parser, command};
use gcode::{pointwise::{surface_mapping::{DepthMapPoint, depth_map_to_transformer}, transformer::transform_gcode_file}, config::MachineConfiguration};
use serde::{Serialize, Deserialize};



#[derive(Serialize, Deserialize)]
#[serde(remote="DepthMapPoint")]
pub struct DepthMapPointSerde {
    pub x: f64,
    pub y: f64,
    pub depth_offset: f64,
}
#[derive(Serialize, Deserialize)]
pub struct DepthMapPointWrapper(#[serde(with="DepthMapPointSerde")] DepthMapPoint);

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// A JSON description of the transformation to apply. May include 'translation' and 'permutation' fields.
    #[arg()]
    depth_file: String,

    /// The file to process.
    #[arg()]
    gcode_file: String,
}

fn main() {
    let args = Args::parse();
    let depth_file: Vec<DepthMapPointWrapper> = serde_json::from_str(&read_to_string(args.depth_file).expect("file didn't open")).expect("file didn't read");
    let depth_file: Vec<_> = depth_file.into_iter().map(|x| x.0).collect();
    let transform = depth_map_to_transformer(depth_file);
    let file = read_to_string(args.gcode_file).unwrap();
    let config = MachineConfiguration::standard_3_axis();
    let result = transform_gcode_file(&config, transform, &file).unwrap();
    println!("{}", result);
}
