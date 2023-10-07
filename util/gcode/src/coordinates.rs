#[derive(Debug, Clone)]
pub struct Position(pub Vec<f64>);
#[derive(Debug, Clone)]
pub struct Offset(pub Vec<f64>);

#[derive(Debug, Clone)]
pub struct PartialPosition(pub Vec<Option<f64>>);
#[derive(Debug, Clone)]
pub struct PartialOffset(pub Vec<Option<f64>>);
