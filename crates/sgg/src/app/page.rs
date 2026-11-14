#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppPage {
    #[default]
    Git,
    Files,
    Settings,
}

/// One step in the app navigation stack (tab + optional preview file).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavEntry {
    pub page: AppPage,
    pub preview: Option<String>,
}

impl NavEntry {
    pub fn page_only(page: AppPage) -> Self {
        Self {
            page,
            preview: None,
        }
    }
}
