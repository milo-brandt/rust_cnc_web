use serde::{Serialize, Deserialize, de::DeserializeOwned};

use crate::generic::{SendableAs, Receivable};

#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct Option<T>(pub std::option::Option<T>);

impl<T: Serialize, U: SendableAs<T>> SendableAs<Option<T>> for std::option::Option<U> {
    fn prepare_in_context(self, context: &crate::communication_context::DeferingContext) -> Option<T> {
        Option(self.map(|value| value.prepare_in_context(context)))
    }
}
impl<T: DeserializeOwned + Receivable> Receivable for Option<T> {
    type ReceivedAs = std::option::Option<T::ReceivedAs>;

    fn receive_in_context(self, context: &crate::communication_context::Context) -> Self::ReceivedAs {
        self.0.map(|value| value.receive_in_context(context))
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct Vec<T>(pub std::vec::Vec<T>);

impl<T: Serialize, U: SendableAs<T>> SendableAs<Vec<T>> for std::vec::Vec<U> {
    fn prepare_in_context(self, context: &crate::communication_context::DeferingContext) -> Vec<T> {
        Vec(self.into_iter().map(|value| value.prepare_in_context(context)).collect())
    }
}
impl<T: DeserializeOwned + Receivable> Receivable for Vec<T> {
    type ReceivedAs = std::vec::Vec<T::ReceivedAs>;

    fn receive_in_context(self, context: &crate::communication_context::Context) -> Self::ReceivedAs {
        self.0.into_iter().map(|value| value.receive_in_context(context)).collect()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct Box<T>(pub std::boxed::Box<T>);

impl<T: Serialize, U: SendableAs<T>> SendableAs<Box<T>> for std::boxed::Box<U> {
    fn prepare_in_context(self, context: &crate::communication_context::DeferingContext) -> Box<T> {
        Box(std::boxed::Box::new((*self).prepare_in_context(context)))
    }
}
impl<T: DeserializeOwned + Receivable> Receivable for Box<T> {
    type ReceivedAs = std::boxed::Box<T::ReceivedAs>;

    fn receive_in_context(self, context: &crate::communication_context::Context) -> Self::ReceivedAs {
        std::boxed::Box::new(self.0.receive_in_context(context))
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct Result<T, E>(pub std::result::Result<T, E>);

impl<T: Serialize, TS: SendableAs<T>, E: Serialize, ES: SendableAs<E>> SendableAs<Result<T, E>> for std::result::Result<TS, ES> {
    fn prepare_in_context(self, context: &crate::communication_context::DeferingContext) -> Result<T, E> {
        Result(self.map(|value| value.prepare_in_context(context)).map_err(|err| err.prepare_in_context(context)))
    }
}
impl<T: DeserializeOwned + Receivable, E: DeserializeOwned + Receivable> Receivable for Result<T, E> {
    type ReceivedAs = std::result::Result<T::ReceivedAs, E::ReceivedAs>;

    fn receive_in_context(self, context: &crate::communication_context::Context) -> Self::ReceivedAs {
        self.0.map(|value| value.receive_in_context(context)).map_err(|err| err.receive_in_context(context))
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct Primitive<T>(pub T);

impl<T: Serialize> SendableAs<Primitive<T>> for T {
    fn prepare_in_context(self, _context: &crate::communication_context::DeferingContext) -> Primitive<T> {
        Primitive(self)
    }
}
impl<T: DeserializeOwned + Send> Receivable for Primitive<T> {
    type ReceivedAs = T;

    fn receive_in_context(self, _context: &crate::communication_context::Context) -> Self::ReceivedAs {
        self.0
    }
}


