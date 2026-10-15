//! Serde types for the DocsAPI config subset the spike reads. Everything else is ignored.
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub document: Document,
    #[serde(rename = "editorConfig")]
    pub editor_config: EditorConfig,
}

#[derive(Debug, Deserialize)]
pub struct Document {
    #[serde(rename = "fileType", default = "docx")]
    pub file_type: String,
    pub key: String,
    #[serde(default)]
    pub title: Option<String>,
    pub url: String,
    #[serde(default)]
    pub permissions: Permissions,
}

#[derive(Debug, Default, Deserialize)]
pub struct Permissions {
    #[serde(default)]
    pub edit: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct EditorConfig {
    #[serde(rename = "callbackUrl")]
    pub callback_url: String,
    #[serde(default = "edit")]
    pub mode: String,
    #[serde(default)]
    pub user: User,
}

#[derive(Debug, Default, Deserialize)]
pub struct User {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub name: String,
}

fn docx() -> String {
    "docx".into()
}
fn edit() -> String {
    "edit".into()
}
