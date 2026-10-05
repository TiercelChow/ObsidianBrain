use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HomeSection<T> {
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HomeOverview {
    pub today: String,
    pub week_start: String,
    pub generated_at: String,
    pub tasks: HomeSection<HomeTasks>,
    pub reading: HomeSection<Vec<HomeBook>>,
    pub memos: HomeSection<HomeMemos>,
    pub wiki: HomeSection<HomeWiki>,
    pub storage: HomeSection<HomeStorage>,
}

#[derive(Debug, Serialize)]
pub struct HomeTasks {
    pub active_count: u64,
    pub overdue_count: u64,
    pub today_count: u64,
    pub items: Vec<HomeTask>,
}

#[derive(Debug, Serialize)]
pub struct HomeTask {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub importance: String,
    pub start_date: String,
    pub end_date: String,
    pub progress_percent: u8,
    pub child_risk_id: Option<String>,
    pub child_risk_title: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HomeBook {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub last_file: Option<String>,
    pub position: f64,
    pub page_count: Option<i64>,
    pub read_at: i64,
}

#[derive(Debug, Serialize)]
pub struct HomeMemos {
    pub total_count: u64,
    pub week_count: u64,
    pub items: Vec<HomeMemo>,
}

#[derive(Debug, Serialize)]
pub struct HomeMemo {
    pub id: String,
    pub date: String,
    pub timestamp: String,
    pub excerpt: String,
    pub thumbnail: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct HomeWiki {
    pub running_count: u64,
    pub review_count: u64,
    pub failed_count: u64,
    pub items: Vec<HomeWikiItem>,
}

#[derive(Debug, Serialize)]
pub struct HomeWikiItem {
    pub id: String,
    pub base_id: String,
    pub book_name: String,
    pub kind: String,
    pub title: String,
    pub status: String,
    pub detail: String,
    pub artifact_state: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct HomeStorage {
    pub originals_bytes: u64,
    pub cache_bytes: u64,
    pub cache_limit_bytes: u64,
    pub pending_cleanup: u64,
}
