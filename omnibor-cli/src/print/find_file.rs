use crate::print::{CommandOutput, Status};
use serde_json::json;
use std::path::PathBuf;
use url::Url;

#[derive(Debug, Clone)]
pub struct FindFileMsg {
    pub path: PathBuf,
    pub id: Url,
}

impl FindFileMsg {
    fn path_string(&self) -> String {
        self.path.display().to_string()
    }

    fn id_string(&self) -> String {
        self.id.to_string()
    }
}

impl CommandOutput for FindFileMsg {
    fn plain_output(&self) -> String {
        format!("{} => {}", self.id_string(), self.path_string())
    }

    fn short_output(&self) -> String {
        self.path_string()
    }

    fn json_output(&self) -> serde_json::Value {
        json!({"path": self.path_string(), "id": self.id_string()})
    }

    fn status(&self) -> Status {
        Status::Success
    }
}