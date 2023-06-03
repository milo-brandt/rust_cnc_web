use std::sync::{Arc};

use async_trait::async_trait;
use axum::{Extension, extract::{FromRequestParts, rejection::ExtensionRejection}};
use hyper::http::request::Parts;
use tokio::sync::{RwLock, RwLockWriteGuard, RwLockReadGuard};
use tower_layer::Layer;

pub struct ExclusiveExtension<T>(Extension<Arc<RwLock<T>>>);

impl<T> Clone for ExclusiveExtension<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> ExclusiveExtension<T> {
    pub fn new(value: T) -> Self {
        Self(Extension(Arc::new(RwLock::new(value))))
    }
    pub async fn write(&self) -> RwLockWriteGuard<T> {
        self.0.write().await
    }
    pub async fn read(&self) -> RwLockReadGuard<T> {
        self.0.read().await
    }
}

impl<S, T> Layer<S> for ExclusiveExtension<T>
where
    Extension<Arc<RwLock<T>>>: Layer<S>
{
    type Service = <Extension<Arc<RwLock<T>>> as Layer<S>>::Service;

    fn layer(&self, inner: S) -> Self::Service {
        self.0.layer(inner)
    }
}

#[async_trait]
impl<S, T> FromRequestParts<S> for ExclusiveExtension<T>
where
    S: Sync,
    Extension<Arc<RwLock<T>>>: FromRequestParts<S>
{
    type Rejection = <Extension<Arc<RwLock<T>>> as FromRequestParts<S>>::Rejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Extension::from_request_parts(parts, state).await.map(|ext| Self(ext))
    }
}