pub mod gcode_motion;
pub mod conversion;
pub mod probe;
pub mod gcode;
pub mod transform;
pub mod coordinates;
pub mod config;
pub mod output;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
