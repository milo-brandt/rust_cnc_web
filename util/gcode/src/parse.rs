use crate::{gcode::{Line, Orientation, LinearMove, CommandContent, HelicalMove, ProbeMove, ModalUpdates, MotionMode, CoordinateMode, Units, ArcPlane}, probe::{ProbeMode, ProbeDirection, ProbeExpectation}, config::MachineConfiguration, coordinates::{PartialPosition, PartialOffset}};

struct Item<'a> {
    head: &'a str,
    value: &'a str
}
// input should be trimmed.
fn parse_item<'a>(input: &'a str) -> Option<(&'a str, Item<'a>)> {
    if input.chars().next().map_or(false, char::is_alphabetic) {
        let head = &input[..1];
        let input = &input[1..];
        let last = input.find(|v: char| !v.is_alphanumeric() && v != '.' && v != '-').unwrap_or(input.len());
        Some((
            &input[last..],
            Item {
                head,
                value: &input[..last],
            }
        ))
    } else {
        None
    }
}
struct ItemSet<'a>(Vec<Item<'a>>);
enum UniquenessIssue { NoneFound, MultipleFound }
impl<'a> ItemSet<'a> {
    pub fn pop_map<T>(&mut self, predicate: impl Fn(&Item<'a>) -> Option<T>) -> Option<T> {
        let (index, result) = self.0.iter().enumerate().find_map(|(index, value)| predicate(value).map(|result| (index, result)))?;
        self.0.remove(index);
        Some(result)
    }
    pub fn pop_map_unique<T>(&mut self, predicate: impl Fn(&Item<'a>) -> Option<T>) -> Result<T, UniquenessIssue> {
        let mut items = self.0.iter().enumerate().filter_map(|(index, value)| predicate(value).map(|result| (index, result)));
        match items.next() {
            Some((index, value)) => match items.next() {
                Some(_) => Err(UniquenessIssue::MultipleFound),
                None => {
                    self.0.remove(index);
                    Ok(value)
                },
            },
            None => Err(UniquenessIssue::NoneFound),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

fn parse_to_items<'a>(mut input: &'a str) -> Option<ItemSet<'a>> {
    let mut items = ItemSet(Vec::new());
    loop {
        input = input.trim_start();
        if input.is_empty() {
            return Some(items)
        }
        let (new_input, item) = parse_item(input)?;
        items.0.push(item);
        input = new_input;
    }
}
enum PrimaryCommand {
    HelicalMove(Orientation),
    ProbeMove(ProbeMode),
}
fn parse_item_set<'a>(config: &MachineConfiguration, mut item_set: ItemSet<'a>) -> Option<Line> {
    let primary_command = item_set.pop_map(|item| {
        if item.head != "G" { return None; }
        match item.value {
            "2" => Some(PrimaryCommand::HelicalMove(Orientation::Clockwise)),
            "3" => Some(PrimaryCommand::HelicalMove(Orientation::Counterclockiwse)),
            "38.2" => Some(PrimaryCommand::ProbeMove(ProbeMode(ProbeDirection::Towards, ProbeExpectation::MustChange))),
            "38.3" => Some(PrimaryCommand::ProbeMove(ProbeMode(ProbeDirection::Towards, ProbeExpectation::MayChange))),
            "38.4" => Some(PrimaryCommand::ProbeMove(ProbeMode(ProbeDirection::Away, ProbeExpectation::MustChange))),
            "38.5" => Some(PrimaryCommand::ProbeMove(ProbeMode(ProbeDirection::Away, ProbeExpectation::MayChange))),
            _ => None
        }
    });
    let axis_words = PartialPosition(config.axis_characters.iter().map(|c| item_set.pop_map(|item|
        if item.head == c.to_string() {
            item.value.parse::<f64>().ok()
        } else {
            None
        }       
    )).collect());
    let command = match primary_command {
        None if axis_words.0.iter().any(Option::is_some) => Some(CommandContent::LinearMove(LinearMove(axis_words))),
        None => None,
        Some(PrimaryCommand::HelicalMove(orientation)) => {
            let mut offset = PartialOffset(config.offset_characters.iter().map(|c| item_set.pop_map(|item|
                if item.head == c.to_string() {
                    item.value.parse::<f64>().ok()
                } else {
                    None
                }
            )).collect());
            offset.0.resize(config.axis_characters.len(), None); // fill in any axes that may not have incremental characters
            Some(CommandContent::HelicalMove(HelicalMove { orientation, target: axis_words, center: offset }))
        },
        Some(PrimaryCommand::ProbeMove(probe_mode)) => Some(
            CommandContent::ProbeMove(ProbeMove(probe_mode, axis_words))
        )
    };
    let modal_updates = ModalUpdates {
        feedrate: item_set.pop_map(|item| if item.head == "F" {
            item.value.parse::<f64>().ok()
        } else {
            None
        }),
        motion_mode: item_set.pop_map(|item| if item.head == "G" {
            match item.value {
                "0" => Some(MotionMode::Rapid),
                "1" => Some(MotionMode::Controlled),
                _ => None,
            }
        } else {
            None
        }),
        coordinate_mode: item_set.pop_map(|item| if item.head == "G" {
            match item.value {
                "90" => Some(CoordinateMode::Absolute),
                _ => None,
            }
        } else {
            None
        }),
        units: item_set.pop_map(|item| if item.head == "G" {
            match item.value {
                "21" => Some(Units::Millimeters),
                _ => None,
            }
        } else {
            None
        }),
        arc_plane: item_set.pop_map(|item| if item.head == "G" {
            config.arc_planes.iter().find_map(|plane| if plane.command_index == item.value {
                Some(ArcPlane(plane.first_axis, plane.second_axis))
            } else {
                None
            })
        } else {
            None
        }),
    };
    if !item_set.is_empty() {
        return None;
    }
    Some(Line {
        modal_updates,
        command,
    })
}
pub fn parse_line(config: &MachineConfiguration, input: &str) -> Option<Line> {
    parse_to_items(input).and_then(|item_set| parse_item_set(config, item_set))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_helix() {
        let config = MachineConfiguration::standard_4_axis();
        let input = "G90 G21 G18 G3 X1.000 Y2.000 Z3.000 A4.000 I5.000 J6.000 F1000.000";
        assert_eq!(
            parse_line(&config, input),
            Some(Line {
                modal_updates: ModalUpdates {
                    feedrate: Some(1000.0),
                    motion_mode: None,
                    coordinate_mode: Some(CoordinateMode::Absolute),
                    units: Some(Units::Millimeters),
                    arc_plane: Some(ArcPlane(2, 0))
                },
                command: Some(CommandContent::HelicalMove(HelicalMove {
                    orientation: Orientation::Counterclockiwse,
                    target: PartialPosition(vec![Some(1.0), Some(2.0), Some(3.0), Some(4.0)]),
                    center: PartialOffset(vec![Some(5.0), Some(6.0), None, None])
                })),
            })
        );
    }
    #[test]
    fn test_simple() {
        let config = MachineConfiguration::standard_4_axis();
        let input = "G0 X1.000 Y2.000 Z3.000";
        assert_eq!(
            parse_line(&config, input),
            Some(Line {
                modal_updates: ModalUpdates {
                    feedrate: None,
                    motion_mode: Some(MotionMode::Rapid),
                    coordinate_mode: None,
                    units: None,
                    arc_plane: None
                },
                command: Some(CommandContent::LinearMove(LinearMove(PartialPosition(vec![Some(1.0), Some(2.0), Some(3.0), None])))),
            })
        );
    }
}