use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BookmarkLocation {
    Epub {
        chapter: String,
        ratio: f64,
    },
    Md {
        ratio: f64,
        block_index: u32,
        text: String,
        offset: f64,
    },
    Pdf {
        page: u32,
        offset: f64,
        x: f64,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BookmarkWrite {
    pub id: Option<String>,
    pub revision: Option<i64>,
    pub book_id: String,
    pub file_path: String,
    pub title: String,
    pub note_md: String,
    pub location: BookmarkLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReaderBookmark {
    pub id: String,
    pub book_id: String,
    pub file_path: String,
    pub title: String,
    pub note_md: String,
    pub location: BookmarkLocation,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}
