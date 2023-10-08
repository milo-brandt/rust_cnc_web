#[derive(Debug, Clone, PartialEq)]
pub struct Position(pub Vec<f64>);
#[derive(Debug, Clone, PartialEq)]
pub struct Offset(pub Vec<f64>);

#[derive(Debug, Clone, PartialEq)]
pub struct PartialPosition(pub Vec<Option<f64>>);
#[derive(Debug, Clone, PartialEq)]
pub struct PartialOffset(pub Vec<Option<f64>>);
