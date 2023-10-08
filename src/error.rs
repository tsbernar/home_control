use thiserror::Error;

#[derive(Error, Debug)]
pub enum HomeError {
    #[error("Error serializing or deserializing, `{0}`")]
    SerdeError(#[from] serde_json::Error),
}
