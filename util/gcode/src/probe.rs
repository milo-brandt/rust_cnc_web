#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum ProbeDirection { Towards, Away }
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum ProbeExpectation { MustChange, MayChange }
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct ProbeMode(pub ProbeDirection, pub ProbeExpectation);
