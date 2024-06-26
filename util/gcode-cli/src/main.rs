use std::collections::HashSet;

use anyhow::{anyhow, Context};
use clap::{Parser, Subcommand, Args};
use gcode::{simple::transform_gcode_file, config::MachineConfiguration, simple::SimpleTransform, simple::SignedIndex, coordinates::{Offset, Sign}, tag::{Tag, tag_gcode_file}, gcode::MachineState, lines::{LinesConfiguration, gcode_file_to_linear}, measure::estimate_extent};
use clap_stdin::FileOrStdin;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[command(subcommand)]
    command: Command
}

#[derive(Subcommand)]
enum Command {
    Transform(TransformationCommand),
    Tag(TagCommand),
    Bounds(BoundsCommand)
}

#[derive(Args)]
struct TransformationCommand {
    /// A JSON description of the transformation to apply. May include 'translation' and 'permutation' fields.
    #[arg()]
    transformation: String,

    /// The file to process.
    #[arg()]
    name: FileOrStdin,
}

#[derive(Args)]
struct TagCommand {
    #[arg()]
    tags: String,

    #[arg()]
    name: FileOrStdin,
}

#[derive(Args)]
struct BoundsCommand {
    #[arg()]
    name: FileOrStdin,
}

#[derive(Serialize, Deserialize, Debug)]
struct TransformationDescription {
    #[serde(default)]
    #[serde(skip_serializing_if="Vec::is_empty")]
    translation: Vec<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if="IndexMap::is_empty")]
    permutation: IndexMap<String, String>,
}

fn string_to_axis(config: &MachineConfiguration, target: &str) -> anyhow::Result<u8> {
    config.axis_characters
        .iter()
        .enumerate()
        .find(|(_, name)| name.to_string() == target)
        .map(|(index, _)| index as u8)
        .ok_or_else(|| anyhow!("Unrecognized axis word: {:?}", target))
}
fn string_to_sign_axis(config: &MachineConfiguration, target: &str) -> anyhow::Result<SignedIndex> {
    let mut iter = target.chars();
    match iter.next() {
        Some('-') => string_to_axis(config, iter.as_str()).map(|v| SignedIndex(Sign::Negative, v)),
        _ => string_to_axis(config, target).map(|v| SignedIndex(Sign::Positive, v))
    }
}
fn map_to_permutation(config: &MachineConfiguration, map: &IndexMap<String, String>) -> anyhow::Result<Vec<SignedIndex>> {
    let mut indices = Vec::new();
    indices.resize(config.axis_characters.len(), None);
    for (key, value) in map.iter() {
        indices[string_to_axis(config, key)? as usize] = Some(string_to_sign_axis(config, value)?);
    }
    let indices: Vec<_> = indices
        .into_iter()
        .enumerate()
        .map(|(index, v)| v.unwrap_or(SignedIndex(Sign::Positive, index as u8)))
        .collect();
    let mut index_set = HashSet::new();
    for SignedIndex(_, index) in &indices {
        if !index_set.insert(index) {
            return Err(anyhow!("Repeated axis word: {:?}", config.axis_characters[*index as usize]));
        }
    }
    Ok(indices)
}
fn description_to_transformation(config: &MachineConfiguration, description: &TransformationDescription) -> anyhow::Result<SimpleTransform> {
    if description.translation.len() > config.axis_characters.len() {
        return Err(anyhow!("Too many terms in translation; expected at most {}, got {}", config.axis_characters.len(), description.translation.len()))
    }
    let mut translation = description.translation.clone();
    translation.resize(config.axis_characters.len(), 0.0);
    let permutation = map_to_permutation(config, &description.permutation)?;
    Ok(SimpleTransform { permutation, offset: Offset(translation) })
}
fn transformation_to_description(config: &MachineConfiguration, transformation: &SimpleTransform) -> String {
    let last_non_zero_translation = transformation.offset.0.iter().enumerate().rfind(|(_, v)| **v != 0.0).map(|(index, _)| index + 1).unwrap_or(0);
    serde_json::to_string(&TransformationDescription {
        translation: transformation.offset.0[..last_non_zero_translation].to_vec(),
        permutation: transformation.permutation.iter().enumerate().filter_map(|(index, output)| {
                if output == &SignedIndex(Sign::Positive, index as u8) {
                    return None
                }
                let prefix = match output.0 {
                    Sign::Positive => "",
                    Sign::Negative => "-",
                };
                Some((
                    config.axis_characters[index].to_string(),
                    format!("{}{}", prefix, config.axis_characters[output.1 as usize].to_string())
                ))
            })
            .collect()
    }).unwrap()
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}
impl<T> OneOrMany<T> {
    pub fn to_vec(self) -> Vec<T> {
        match self {
            OneOrMany::One(value) => vec![value],
            OneOrMany::Many(value) => value,
        }
    }
}

#[derive(Deserialize)]
pub struct TagDescription {
    position: (f64, f64),
    radius: f64,
    minimum_height: f64,
}

fn main() {
    let args = CliArgs::parse();
    match args.command {
        Command::Transform(args) => {
            let transforms = serde_json::from_str::<OneOrMany<TransformationDescription>>(&args.transformation).context("Failed while parsing JSON input").unwrap().to_vec();
            let machine_config = MachineConfiguration::standard_4_axis();
            let transforms = transforms
                .iter()
                .enumerate()
                .map(|(index, transform)|
                    description_to_transformation(&machine_config, &transform).context(format!("Provided transformation index {} is invalid.", index))
                )
                .collect::<Result<Vec<_>, _>>().unwrap();

            let result = transforms.iter().map(|transform| -> anyhow::Result<_> {
                let result = transform_gcode_file(&machine_config, &transform, &args.name)
                    .map_err(|e| anyhow!("Failed to apply transformation! Error on line {}", e + 1))?;
                Ok(format!("(START TRANSFORM: {})\n{}(END TRANSFORM)\n", transformation_to_description(&machine_config, transform), result))
            }).collect::<anyhow::Result<String>>().unwrap();

            println!("{}", result);
        }
        Command::Tag(args) => {
            let tags = serde_json::from_str::<Vec<TagDescription>>(&args.tags).context("Failed while parsing JSON input").unwrap();
            let tags: Vec<_> = tags
                .iter()
                .map(|tag|
                    Tag {
                        position: tag.position,
                        minimum_height: tag.minimum_height,
                        radius: tag.radius,
                    }
                ).collect();
            let mut result = gcode_file_to_linear(
                &MachineConfiguration::standard_4_axis(),
                MachineState::new(4),
                &LinesConfiguration {
                    tolerance: 0.01,
                    arc_radii_tolerance: 0.01,
                },
                &args.name
            ).map_err(|e| anyhow!("Failed to linearlize line {}", e)).unwrap();
            for tag in tags {
                result = tag_gcode_file(
                    &MachineConfiguration::standard_4_axis(),
                    MachineState::new(4),
                    tag,
                    &result
                ).map_err(|e| anyhow!("Failed to tag line {}", e)).unwrap();
            }

            println!("{}", result);
        }
        Command::Bounds(args) => {
            let bounds = estimate_extent(
                &MachineConfiguration::standard_4_axis(), &args.name
            ).map_err(|e| anyhow!("Failed on line {}", e)).unwrap();
            println!("{}", serde_json::to_string_pretty(&bounds.bounds).unwrap())
        }
    }
}
