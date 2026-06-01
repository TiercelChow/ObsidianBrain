//! 代码仓管理模块

pub mod git_extractor;
pub mod language_detect;
pub mod manager;
pub mod note_linker;
pub mod vscode;

#[allow(unused_imports)]
pub use git_extractor::GitExtractor;
#[allow(unused_imports)]
pub use language_detect::LanguageDetector;
#[allow(unused_imports)]
pub use manager::RepoManager;
#[allow(unused_imports)]
pub use note_linker::NoteLinker;
#[allow(unused_imports)]
pub use vscode::VscodeOpener;
