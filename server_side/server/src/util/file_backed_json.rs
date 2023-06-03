use std::path::PathBuf;

use serde::{Serialize, de::DeserializeOwned};
use tokio::fs::{File, read_to_string, write, create_dir_all};

pub struct FileBackedValue<T> {
    value: T,
    path: PathBuf,
}
impl<T: Serialize + DeserializeOwned> FileBackedValue<T> {
    pub async fn new(path: PathBuf, default: impl FnOnce() -> T) -> anyhow::Result<FileBackedValue<T>> {
        // First: look for an existing value; if not found, make sure the directory exists and return the default value
        let value = if path.exists() {
            serde_json::from_str(&read_to_string(&path).await?)?
        } else {
            if let Some(parent) = path.parent() {
                // Make sure the parent directory exists...
                create_dir_all(parent).await?;
            }
            default()
        };
        Ok(FileBackedValue {
            value,
            path
        })
    }
    pub fn get(&self) -> &T {
        &self.value
    }
    pub async fn mutate<R>(&mut self, f: impl FnOnce(&mut T) -> anyhow::Result<R>) -> anyhow::Result<R> {
        let result = f(&mut self.value)?;
        write(
            &self.path,
            serde_json::to_string(&self.value)?
        ).await?;
        Ok(result)
    }
    pub async fn set(&mut self, value: T) -> anyhow::Result<()> {
        self.mutate(|inner| {
            *inner = value;
            Ok(())
        }).await
    }
}