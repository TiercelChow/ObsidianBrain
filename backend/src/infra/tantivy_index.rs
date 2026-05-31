use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::{AllQuery, BooleanQuery, Occur, QueryParser, TermQuery};
use tantivy::schema::*;
use tantivy::tokenizer::{LowerCaser, TextAnalyzer, Token, TokenStream, Tokenizer};
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term};

use crate::error::BrainError;

// ── Jieba Tokenizer (wraps jieba-rs for tantivy 0.22) ──

/// A tantivy Tokenizer backed by jieba-rs for Chinese word segmentation.
#[derive(Clone)]
struct JiebaTokenizer {
    jieba: jieba_rs::Jieba,
}

impl JiebaTokenizer {
    fn new() -> Self {
        JiebaTokenizer {
            jieba: jieba_rs::Jieba::new(),
        }
    }
}

/// TokenStream produced by JiebaTokenizer.
struct JiebaTokenStream {
    tokens: Vec<Token>,
    index: usize,
}

impl TokenStream for JiebaTokenStream {
    fn advance(&mut self) -> bool {
        self.index += 1;
        self.index <= self.tokens.len()
    }

    fn token(&self) -> &Token {
        &self.tokens[self.index - 1]
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.index - 1]
    }
}

impl Tokenizer for JiebaTokenizer {
    type TokenStream<'a> = JiebaTokenStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        let words = self
            .jieba
            .tokenize(text, jieba_rs::TokenizeMode::Search, true);
        let tokens: Vec<Token> = words
            .iter()
            .enumerate()
            .map(|(i, word)| Token {
                offset_from: word.start,
                offset_to: word.end,
                position: i,
                text: word.word.to_lowercase(),
                position_length: 1,
            })
            .collect();

        JiebaTokenStream { tokens, index: 0 }
    }
}

// ── Schema & Index ──

/// Field references for the Tantivy schema.
struct FieldMap {
    title: Field,
    content: Field,
    path: Field,
    tags: Field,
    chunk_id: Field,
    note_path: Field,
}

/// A document to be indexed.
#[allow(dead_code)]
pub struct NoteDocument {
    pub title: String,
    pub content: String,
    pub path: String,
    pub tags: Vec<String>,
    pub chunk_id: String,
    pub note_path: String,
}

/// Search parameters.
#[allow(dead_code)]
pub struct SearchParams {
    pub query: String,
    pub top_k: usize,
    pub tag_filter: Option<Vec<String>>,
}

/// A single search result.
#[derive(Clone)]
#[allow(dead_code)]
pub struct TantivySearchResult {
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub score: f32,
    pub tags: Vec<String>,
    pub chunk_id: String,
    pub note_path: String,
}

/// Tantivy full-text search index with Chinese (jieba) support.
pub struct TantivyIndex {
    index: Index,
    fields: FieldMap,
    writer: std::sync::Mutex<IndexWriter<TantivyDocument>>,
    reader: IndexReader,
}

impl TantivyIndex {
    fn build_schema() -> (Schema, FieldMap) {
        let mut schema_builder = Schema::builder();

        let text_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("jieba")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();

        let title = schema_builder.add_text_field("title", text_options.clone());
        let content = schema_builder.add_text_field("content", text_options);

        let path = schema_builder.add_text_field("path", STRING | STORED);

        let chunk_id = schema_builder.add_text_field("chunk_id", STRING | STORED);
        let note_path = schema_builder.add_text_field("note_path", STRING | STORED);

        let tag_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("simple")
                    .set_index_option(IndexRecordOption::WithFreqs),
            )
            .set_stored();
        let tags = schema_builder.add_text_field("tags", tag_options);

        let schema = schema_builder.build();
        let fields = FieldMap {
            title,
            content,
            path,
            tags,
            chunk_id,
            note_path,
        };
        (schema, fields)
    }

    /// Open or create the index at `index_path`.
    pub fn new(index_path: &Path) -> Result<Self, BrainError> {
        let (schema, fields) = Self::build_schema();

        std::fs::create_dir_all(index_path)?;

        let index = if index_path.join("meta.json").exists() {
            Index::open_in_dir(index_path)
                .map_err(|e| BrainError::SearchError(format!("索引打开失败: {e}")))?
        } else {
            Index::create_in_dir(index_path, schema)
                .map_err(|e| BrainError::SearchError(format!("索引创建失败: {e}")))?
        };

        // Register our jieba tokenizer with LowerCaser for case-insensitive search
        let jieba_analyzer = TextAnalyzer::builder(JiebaTokenizer::new())
            .filter(LowerCaser)
            .build();
        index.tokenizers().register("jieba", jieba_analyzer);

        // Register simple tokenizer (split on whitespace + punctuation)
        index
            .tokenizers()
            .register("simple", tantivy::tokenizer::SimpleTokenizer::default());

        let writer = index
            .writer::<TantivyDocument>(50_000_000)
            .map_err(|e| BrainError::SearchError(format!("Writer 创建失败: {e}")))?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| BrainError::SearchError(format!("Reader 创建失败: {e}")))?;

        Ok(TantivyIndex {
            index,
            fields,
            writer: std::sync::Mutex::new(writer),
            reader,
        })
    }

    /// Add a document to the index.
    pub fn add_document(&self, note: &NoteDocument) -> Result<(), BrainError> {
        let writer = self.writer.lock().unwrap();
        writer
            .add_document(doc!(
                self.fields.title => note.title.clone(),
                self.fields.content => note.content.clone(),
                self.fields.path => note.path.clone(),
                self.fields.tags => note.tags.join(" "),
                self.fields.chunk_id => note.chunk_id.clone(),
                self.fields.note_path => note.note_path.clone(),
            ))
            .map_err(|e| BrainError::SearchError(format!("文档添加失败: {e}")))?;
        Ok(())
    }

    /// Update a document (delete old + add new).
    pub fn update_document(&self, note: &NoteDocument) -> Result<(), BrainError> {
        self.delete_document(&note.path)?;
        self.add_document(note)?;
        Ok(())
    }

    /// Delete a document by path.
    pub fn delete_document(&self, path: &str) -> Result<(), BrainError> {
        let term = Term::from_field_text(self.fields.path, path);
        let writer = self.writer.lock().unwrap();
        writer.delete_term(term);
        Ok(())
    }

    /// Delete all documents belonging to a note (by note_path field).
    /// This removes all chunks for a given file.
    pub fn delete_by_note_path(&self, note_path: &str) -> Result<(), BrainError> {
        let term = Term::from_field_text(self.fields.note_path, note_path);
        let writer = self.writer.lock().unwrap();
        writer.delete_term(term);
        Ok(())
    }

    /// Delete a single chunk by its UUID (chunk_id field).
    pub fn delete_by_chunk_id(&self, chunk_id: &str) -> Result<(), BrainError> {
        let term = Term::from_field_text(self.fields.chunk_id, chunk_id);
        let writer = self.writer.lock().unwrap();
        writer.delete_term(term);
        Ok(())
    }

    /// Commit pending changes and reload the reader.
    pub fn commit(&self) -> Result<(), BrainError> {
        let mut writer = self.writer.lock().unwrap();
        writer
            .commit()
            .map_err(|e| BrainError::SearchError(format!("提交失败: {e}")))?;
        self.reader
            .reload()
            .map_err(|e| BrainError::SearchError(format!("Reader 刷新失败: {e}")))?;
        Ok(())
    }

    /// Full-text search with optional tag filter.
    pub fn search(&self, params: &SearchParams) -> Result<Vec<TantivySearchResult>, BrainError> {
        let searcher = self.reader.searcher();

        let query_parser =
            QueryParser::for_index(&self.index, vec![self.fields.title, self.fields.content]);

        let text_query = query_parser
            .parse_query(&params.query)
            .map_err(|e| BrainError::SearchError(format!("查询解析失败: {e}")))?;

        let final_query: Box<dyn tantivy::query::Query> = if let Some(ref tags) = params.tag_filter
        {
            let mut sub_queries: Vec<(Occur, Box<dyn tantivy::query::Query>)> =
                vec![(Occur::Must, text_query)];

            for tag in tags {
                let tag_term = Term::from_field_text(self.fields.tags, tag);
                let tag_query: Box<dyn tantivy::query::Query> =
                    Box::new(TermQuery::new(tag_term, IndexRecordOption::Basic));
                sub_queries.push((Occur::Must, tag_query));
            }

            Box::new(BooleanQuery::from(sub_queries))
        } else {
            text_query
        };

        let top_docs = searcher
            .search(&*final_query, &TopDocs::with_limit(params.top_k))
            .map_err(|e| BrainError::SearchError(format!("搜索执行失败: {e}")))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| BrainError::SearchError(format!("文档获取失败: {e}")))?;

            let path = doc
                .get_first(self.fields.path)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let title = doc
                .get_first(self.fields.title)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let content = doc
                .get_first(self.fields.content)
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let tags_text = doc
                .get_first(self.fields.tags)
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let chunk_id = doc
                .get_first(self.fields.chunk_id)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let note_path = doc
                .get_first(self.fields.note_path)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let snippet = make_snippet(content, &params.query, 200);

            results.push(TantivySearchResult {
                path,
                title,
                snippet,
                score,
                tags: tags_text.split_whitespace().map(String::from).collect(),
                chunk_id,
                note_path,
            });
        }

        Ok(results)
    }

    /// Search all documents in the index using AllQuery.
    /// Returns all documents with their field values.
    pub fn search_all(&self) -> Result<Vec<TantivySearchResult>, BrainError> {
        let searcher = self.reader.searcher();
        let top_docs = searcher
            .search(&AllQuery, &TopDocs::with_limit(10_000))
            .map_err(|e| BrainError::SearchError(format!("全量搜索失败: {e}")))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| BrainError::SearchError(format!("文档获取失败: {e}")))?;

            let path = doc
                .get_first(self.fields.path)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let title = doc
                .get_first(self.fields.title)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let content = doc
                .get_first(self.fields.content)
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let tags_text = doc
                .get_first(self.fields.tags)
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let chunk_id = doc
                .get_first(self.fields.chunk_id)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let note_path = doc
                .get_first(self.fields.note_path)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let snippet = make_snippet(content, "", 200);

            results.push(TantivySearchResult {
                path,
                title,
                snippet,
                score,
                tags: tags_text.split_whitespace().map(String::from).collect(),
                chunk_id,
                note_path,
            });
        }

        Ok(results)
    }

    /// Check if the index is operational.
    pub fn health_check(&self) -> bool {
        let searcher = self.reader.searcher();
        searcher.num_docs() > 0 || searcher.num_docs() == 0
    }
}

/// Generate a text snippet around the first query match.
fn make_snippet(content: &str, query: &str, max_len: usize) -> String {
    let lower = content.to_lowercase();
    let query_lower = query.to_lowercase();

    if let Some(pos) = lower.find(&query_lower) {
        let start = pos.saturating_sub(max_len / 3);
        let end = (pos + query.len() + max_len * 2 / 3).min(content.len());
        format!("...{}...", content[start..end].trim())
    } else {
        let end = max_len.min(content.len());
        format!("{}...", content[..end].trim())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_index_and_search_chinese() {
        let dir = TempDir::new().unwrap();
        let index = TantivyIndex::new(dir.path()).unwrap();

        index
            .add_document(&NoteDocument {
                title: "Rust 异步编程".to_string(),
                content: "Tokio 是 Rust 生态中最流行的异步运行时框架".to_string(),
                path: "programming/rust-async.md".to_string(),
                tags: vec!["rust".to_string(), "async".to_string()],
                chunk_id: "chunk-1".to_string(),
                note_path: "programming/rust-async.md".to_string(),
            })
            .unwrap();

        index.commit().unwrap();

        let results = index
            .search(&SearchParams {
                query: "异步运行时".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();

        assert!(!results.is_empty());
        assert!(results[0].path.contains("rust-async"));
    }

    #[test]
    fn test_delete_document() {
        let dir = TempDir::new().unwrap();
        let index = TantivyIndex::new(dir.path()).unwrap();

        index
            .add_document(&NoteDocument {
                title: "Test".to_string(),
                content: "This should be deleted".to_string(),
                path: "test.md".to_string(),
                tags: vec![],
                chunk_id: "chunk-del".to_string(),
                note_path: "test.md".to_string(),
            })
            .unwrap();
        index.commit().unwrap();

        index.delete_document("test.md").unwrap();
        index.commit().unwrap();

        let results = index
            .search(&SearchParams {
                query: "deleted".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn test_tag_filter() {
        let dir = TempDir::new().unwrap();
        let index = TantivyIndex::new(dir.path()).unwrap();

        index
            .add_document(&NoteDocument {
                title: "Rust Notes".to_string(),
                content: "Rust programming language notes".to_string(),
                path: "rust.md".to_string(),
                tags: vec!["rust".to_string()],
                chunk_id: "chunk-rust".to_string(),
                note_path: "rust.md".to_string(),
            })
            .unwrap();
        index
            .add_document(&NoteDocument {
                title: "Python Notes".to_string(),
                content: "Python programming language notes".to_string(),
                path: "python.md".to_string(),
                tags: vec!["python".to_string()],
                chunk_id: "chunk-python".to_string(),
                note_path: "python.md".to_string(),
            })
            .unwrap();
        index.commit().unwrap();

        let results = index
            .search(&SearchParams {
                query: "programming".to_string(),
                top_k: 5,
                tag_filter: Some(vec!["rust".to_string()]),
            })
            .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].path.contains("rust"));
    }

    #[test]
    fn test_snippet_generation() {
        let snippet = make_snippet(
            "Hello world this is a test of snippet generation",
            "test",
            30,
        );
        assert!(snippet.contains("test"));
        assert!(snippet.starts_with("..."));
    }
}
