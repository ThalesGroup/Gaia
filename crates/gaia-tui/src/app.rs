use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use gaia_core::backend::{BackendAvailability, backend_from_name};
use gaia_core::machine::MachineSpecs;
use gaia_core::model_catalog::ModelCatalog;
use gaia_core::recommendation::{FitStatus, PreferredBackend, RecommendationEngine};

#[derive(Debug, Clone)]
pub struct SelectorInput {
    pub machine: MachineSpecs,
    pub catalog: ModelCatalog,
    pub default_backend: String,
    pub default_model: Option<String>,
    pub default_port: u16,
    pub default_api_key: String,
}

#[derive(Debug, Clone)]
pub struct SelectorResult {
    pub backend: String,
    pub model_id: String,
    pub port: u16,
    pub api_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardMode {
    Browse,
    Search,
    CategoryPicker,
    SizePicker,
    PortInput,
    ApiKeyInput,
    Confirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeFilter {
    All,
    UpTo8B,
    UpTo14B,
    UpTo32B,
}

impl SizeFilter {
    pub fn all() -> [Self; 4] {
        [Self::All, Self::UpTo8B, Self::UpTo14B, Self::UpTo32B]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All sizes",
            Self::UpTo8B => "<= 8B",
            Self::UpTo14B => "<= 14B",
            Self::UpTo32B => "<= 32B",
        }
    }

    pub fn allows(self, params_b: f32) -> bool {
        match self {
            Self::All => true,
            Self::UpTo8B => params_b <= 8.0,
            Self::UpTo14B => params_b <= 14.0,
            Self::UpTo32B => params_b <= 32.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BackendChoice {
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Clone)]
pub struct ModelItem {
    pub id: String,
    pub display_name: String,
    pub family: String,
    pub params_b: f32,
    pub categories: Vec<String>,
    pub recommended_use: String,
    pub min_vram_gb_fp16: f32,
    pub min_vram_gb_int8: f32,
    pub min_vram_gb_int4: f32,
    pub supports_vllm: bool,
    pub supports_tgi: bool,
    pub supports_sglang: bool,
    pub supports_llamacpp: bool,
    pub supports_ollama: bool,
    pub gated: bool,
    pub fit: FitStatus,
    pub explanation: String,
    pub recommended: bool,
}

#[derive(Debug, Clone)]
pub enum AppAction {
    None,
    Cancelled,
    Confirmed(SelectorResult),
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub machine: MachineSpecs,
    pub mode: WizardMode,
    pub status_message: String,
    pub spinner_tick: usize,
    pub preferred_backend: PreferredBackend,
    pub backend_choices: Vec<BackendChoice>,
    pub backend_availability: Vec<BackendAvailability>,
    pub backend_index: usize,
    pub models: Vec<ModelItem>,
    pub selected_model_index: usize,
    pub search_query: String,
    pub search_input: String,
    pub category_options: Vec<String>,
    pub category_picker_index: usize,
    pub selected_category: Option<String>,
    pub size_filter: SizeFilter,
    pub size_picker_index: usize,
    pub port_input: String,
    pub api_key_input: String,
    pub model_details_scroll: u16,
    pub details_focus: bool,
    catalog: ModelCatalog,
}

impl AppState {
    pub fn new(input: SelectorInput) -> Self {
        let backend_choices = vec![
            BackendChoice {
                id: "vllm",
                label: "vLLM",
            },
            BackendChoice {
                id: "tgi",
                label: "TGI",
            },
            BackendChoice {
                id: "sglang",
                label: "SGLang",
            },
            BackendChoice {
                id: "llamacpp",
                label: "llama.cpp",
            },
            BackendChoice {
                id: "ollama",
                label: "Ollama",
            },
        ];

        let backend_availability = backend_choices
            .iter()
            .map(|choice| {
                backend_from_name(choice.id)
                    .map(|backend| backend.is_available(&input.machine))
                    .unwrap_or_else(|| BackendAvailability::unavailable("Backend unavailable."))
            })
            .collect::<Vec<_>>();

        let preferred_backend = RecommendationEngine::preferred_backend(&input.machine);
        let backend_index =
            resolve_backend_index(&input.default_backend, preferred_backend, &backend_choices);

        let mut categories = input
            .catalog
            .models
            .iter()
            .flat_map(|model| model.categories.iter().cloned())
            .collect::<Vec<_>>();
        categories.sort();
        categories.dedup();
        categories.insert(0, "all".to_owned());

        let mut app = Self {
            machine: input.machine,
            mode: WizardMode::Browse,
            status_message: "Use arrows to explore models, then press Enter.".to_owned(),
            spinner_tick: 0,
            preferred_backend,
            backend_choices,
            backend_availability,
            backend_index,
            models: Vec::new(),
            selected_model_index: 0,
            search_query: String::new(),
            search_input: String::new(),
            category_options: categories,
            category_picker_index: 0,
            selected_category: None,
            size_filter: SizeFilter::All,
            size_picker_index: 0,
            port_input: input.default_port.to_string(),
            api_key_input: input.default_api_key,
            model_details_scroll: 0,
            details_focus: false,
            catalog: input.catalog,
        };

        app.rebuild_models(input.default_model.as_deref());
        app
    }

    pub fn on_tick(&mut self) {
        self.spinner_tick = self.spinner_tick.wrapping_add(1);
    }

    pub fn current_backend(&self) -> BackendChoice {
        self.backend_choices[self.backend_index]
    }

    pub fn current_backend_availability(&self) -> &BackendAvailability {
        &self.backend_availability[self.backend_index]
    }

    pub fn selected_model(&self) -> Option<&ModelItem> {
        self.models.get(self.selected_model_index)
    }

    pub fn selected_model_hf_url(&self) -> Option<String> {
        self.selected_model()
            .map(|model| format!("https://hf.co/{}", model.id))
    }

    pub fn category_label(&self) -> &str {
        self.selected_category.as_deref().unwrap_or("all")
    }

    pub fn size_label(&self) -> &'static str {
        self.size_filter.label()
    }

    pub fn is_backend_preferred(&self, backend_id: &str) -> bool {
        self.preferred_backend
            .as_str()
            .is_some_and(|preferred| preferred == backend_id)
    }

    pub fn handle_key(&mut self, key_event: KeyEvent) -> AppAction {
        match self.mode {
            WizardMode::Browse => self.handle_browse_key(key_event),
            WizardMode::Search => self.handle_search_key(key_event),
            WizardMode::CategoryPicker => self.handle_category_picker_key(key_event),
            WizardMode::SizePicker => self.handle_size_picker_key(key_event),
            WizardMode::PortInput => self.handle_port_input_key(key_event),
            WizardMode::ApiKeyInput => self.handle_api_key_input_key(key_event),
            WizardMode::Confirm => self.handle_confirm_key(key_event),
        }
    }

    fn handle_browse_key(&mut self, key_event: KeyEvent) -> AppAction {
        if self.details_focus {
            match key_event.code {
                KeyCode::Char(' ') => {
                    self.details_focus = false;
                    self.status_message = "Model list focus restored.".to_owned();
                }
                KeyCode::Esc => {
                    self.details_focus = false;
                    self.status_message = "Details focus disabled.".to_owned();
                }
                KeyCode::Char('q') => return AppAction::Cancelled,
                KeyCode::Up => {
                    self.model_details_scroll = self.model_details_scroll.saturating_sub(1);
                }
                KeyCode::Down => {
                    self.model_details_scroll = self.model_details_scroll.saturating_add(1);
                }
                KeyCode::PageUp => {
                    self.model_details_scroll = self.model_details_scroll.saturating_sub(4);
                }
                KeyCode::PageDown => {
                    self.model_details_scroll = self.model_details_scroll.saturating_add(4);
                }
                KeyCode::Char('u') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.model_details_scroll = self.model_details_scroll.saturating_sub(1);
                }
                KeyCode::Char('d') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.model_details_scroll = self.model_details_scroll.saturating_add(1);
                }
                _ => {
                    self.status_message =
                        "Details focus active. Press Space to return to model list.".to_owned();
                }
            }
            return AppAction::None;
        }

        match key_event.code {
            KeyCode::Char(' ') => {
                self.details_focus = true;
                self.status_message =
                    "Details focus enabled: Up/Down scroll details, Space to return.".to_owned();
            }
            KeyCode::Up if self.selected_model_index > 0 => {
                self.selected_model_index -= 1;
                self.model_details_scroll = 0;
            }
            KeyCode::Down if self.selected_model_index + 1 < self.models.len() => {
                self.selected_model_index += 1;
                self.model_details_scroll = 0;
            }
            KeyCode::Char('b') => {
                self.backend_index = (self.backend_index + 1) % self.backend_choices.len();
                self.rebuild_models(None);
                self.status_message =
                    format!("Backend switched to {}.", self.current_backend().label);
            }
            KeyCode::Char('x') => {
                if self.search_query.is_empty() {
                    self.status_message = "No active search filter.".to_owned();
                } else {
                    self.clear_search();
                }
            }
            KeyCode::Char('/') => {
                self.search_input = self.search_query.clone();
                self.mode = WizardMode::Search;
                self.status_message = "Search mode: type and press Enter.".to_owned();
            }
            KeyCode::Char('f') | KeyCode::Char('c') => {
                self.category_picker_index = self
                    .selected_category
                    .as_ref()
                    .and_then(|selected| {
                        self.category_options
                            .iter()
                            .position(|category| category == selected)
                    })
                    .unwrap_or(0);
                self.mode = WizardMode::CategoryPicker;
                self.status_message = "Category filter opened.".to_owned();
            }
            KeyCode::Char('s') => {
                self.size_picker_index = SizeFilter::all()
                    .iter()
                    .position(|candidate| *candidate == self.size_filter)
                    .unwrap_or(0);
                self.mode = WizardMode::SizePicker;
                self.status_message = "Size filter opened.".to_owned();
            }
            KeyCode::Enter => {
                if self.models.is_empty() {
                    self.status_message = "No model matches current filters.".to_owned();
                } else {
                    self.mode = WizardMode::PortInput;
                    self.status_message = "Enter API port, then press Enter.".to_owned();
                }
            }
            KeyCode::Esc => {
                if self.search_query.is_empty() {
                    return AppAction::Cancelled;
                }
                self.clear_search();
            }
            KeyCode::Char('q') => return AppAction::Cancelled,
            _ => {}
        }
        AppAction::None
    }

    fn handle_search_key(&mut self, key_event: KeyEvent) -> AppAction {
        match key_event.code {
            KeyCode::Esc => {
                self.mode = WizardMode::Browse;
                self.status_message = "Search cancelled.".to_owned();
            }
            KeyCode::Enter => {
                self.search_query = self.search_input.trim().to_owned();
                self.rebuild_models(None);
                self.mode = WizardMode::Browse;
                self.status_message = if self.search_query.is_empty() {
                    "Search cleared.".to_owned()
                } else {
                    format!("Search applied: {}", self.search_query)
                };
            }
            KeyCode::Backspace => {
                self.search_input.pop();
            }
            KeyCode::Char(ch) if !key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.search_input.push(ch);
            }
            _ => {}
        }
        AppAction::None
    }

    fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_input.clear();
        self.rebuild_models(None);
        self.status_message = "Search cleared, back to full list.".to_owned();
    }

    fn handle_category_picker_key(&mut self, key_event: KeyEvent) -> AppAction {
        match key_event.code {
            KeyCode::Esc => {
                self.mode = WizardMode::Browse;
                self.status_message = "Category filter unchanged.".to_owned();
            }
            KeyCode::Up if self.category_picker_index > 0 => {
                self.category_picker_index -= 1;
            }
            KeyCode::Down if self.category_picker_index + 1 < self.category_options.len() => {
                self.category_picker_index += 1;
            }
            KeyCode::Enter => {
                let selected = self.category_options[self.category_picker_index].clone();
                self.selected_category = if selected == "all" {
                    None
                } else {
                    Some(selected.clone())
                };
                self.rebuild_models(None);
                self.mode = WizardMode::Browse;
                self.status_message = format!("Category filter: {selected}");
            }
            _ => {}
        }
        AppAction::None
    }

    fn handle_size_picker_key(&mut self, key_event: KeyEvent) -> AppAction {
        let options = SizeFilter::all();
        match key_event.code {
            KeyCode::Esc => {
                self.mode = WizardMode::Browse;
                self.status_message = "Size filter unchanged.".to_owned();
            }
            KeyCode::Up if self.size_picker_index > 0 => {
                self.size_picker_index -= 1;
            }
            KeyCode::Down if self.size_picker_index + 1 < options.len() => {
                self.size_picker_index += 1;
            }
            KeyCode::Enter => {
                self.size_filter = options[self.size_picker_index];
                self.rebuild_models(None);
                self.mode = WizardMode::Browse;
                self.status_message = format!("Size filter: {}", self.size_filter.label());
            }
            _ => {}
        }
        AppAction::None
    }

    fn handle_port_input_key(&mut self, key_event: KeyEvent) -> AppAction {
        match key_event.code {
            KeyCode::Esc => {
                self.mode = WizardMode::Browse;
                self.status_message = "Port input cancelled.".to_owned();
            }
            KeyCode::Backspace => {
                self.port_input.pop();
            }
            KeyCode::Char(ch) if ch.is_ascii_digit() && self.port_input.len() < 5 => {
                self.port_input.push(ch);
            }
            KeyCode::Enter => {
                if parse_port(&self.port_input).is_some() {
                    self.mode = WizardMode::ApiKeyInput;
                    self.status_message = "Set API key or leave empty for local-key.".to_owned();
                } else {
                    self.status_message =
                        "Invalid port. Please enter a value between 1 and 65535.".to_owned();
                }
            }
            _ => {}
        }
        AppAction::None
    }

    fn handle_api_key_input_key(&mut self, key_event: KeyEvent) -> AppAction {
        match key_event.code {
            KeyCode::Esc => {
                self.mode = WizardMode::PortInput;
                self.status_message = "Back to port configuration.".to_owned();
            }
            KeyCode::Backspace => {
                self.api_key_input.pop();
            }
            KeyCode::Enter => {
                if self.api_key_input.trim().is_empty() {
                    self.api_key_input = "local-key".to_owned();
                }
                self.mode = WizardMode::Confirm;
                self.status_message = "Review and confirm launch.".to_owned();
            }
            KeyCode::Char('g') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.api_key_input = "local-key".to_owned();
                self.status_message =
                    "API key generated (local-key). Press Enter to continue.".to_owned();
            }
            KeyCode::Char(ch) if !key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.api_key_input.push(ch);
            }
            _ => {}
        }
        AppAction::None
    }

    fn handle_confirm_key(&mut self, key_event: KeyEvent) -> AppAction {
        match key_event.code {
            KeyCode::Esc => {
                self.mode = WizardMode::Browse;
                self.status_message =
                    "Confirmation cancelled. You can adjust selections.".to_owned();
                AppAction::None
            }
            KeyCode::Char('q') => AppAction::Cancelled,
            KeyCode::Enter => {
                if let Some(result) = self.build_result() {
                    AppAction::Confirmed(result)
                } else {
                    self.status_message = "Unable to launch: invalid selections.".to_owned();
                    AppAction::None
                }
            }
            _ => AppAction::None,
        }
    }

    fn build_result(&self) -> Option<SelectorResult> {
        let model = self.selected_model()?;
        let port = parse_port(&self.port_input)?;
        let api_key = if self.api_key_input.trim().is_empty() {
            "local-key".to_owned()
        } else {
            self.api_key_input.trim().to_owned()
        };

        Some(SelectorResult {
            backend: self.current_backend().id.to_owned(),
            model_id: model.id.clone(),
            port,
            api_key,
        })
    }

    fn rebuild_models(&mut self, prefer_model_id: Option<&str>) {
        let previous_selection = self.selected_model().map(|model| model.id.clone());
        let backend = self.current_backend().id;
        let query = self.search_query.to_lowercase();

        let mut positive_rank = 0usize;
        let mut models =
            RecommendationEngine::recommend_models(&self.machine, &self.catalog, Some(backend))
                .into_iter()
                .filter(|rec| {
                    if let Some(category) = &self.selected_category {
                        rec.model.categories.iter().any(|item| item == category)
                    } else {
                        true
                    }
                })
                .filter(|rec| self.size_filter.allows(rec.model.params_b))
                .filter(|rec| {
                    if query.is_empty() {
                        return true;
                    }

                    let searchable = format!(
                        "{} {} {} {}",
                        rec.model.id,
                        rec.model.display_name,
                        rec.model.family,
                        rec.model.categories.join(" ")
                    )
                    .to_lowercase();
                    searchable.contains(&query)
                })
                .map(|rec| {
                    let recommended = rec.status.is_positive() && positive_rank < 5;
                    if rec.status.is_positive() {
                        positive_rank += 1;
                    }
                    let supports_sglang = rec.model.supports_backend("sglang");
                    let supports_llamacpp = rec.model.supports_backend("llamacpp");
                    let supports_ollama = rec.model.supports_backend("ollama");

                    ModelItem {
                        id: rec.model.id,
                        display_name: rec.model.display_name,
                        family: rec.model.family,
                        params_b: rec.model.params_b,
                        categories: rec.model.categories,
                        recommended_use: rec.model.recommended_use,
                        min_vram_gb_fp16: rec.model.min_vram_gb_fp16,
                        min_vram_gb_int8: rec.model.min_vram_gb_int8,
                        min_vram_gb_int4: rec.model.min_vram_gb_int4,
                        supports_vllm: rec.model.supports_vllm,
                        supports_tgi: rec.model.supports_tgi,
                        supports_sglang,
                        supports_llamacpp,
                        supports_ollama,
                        gated: rec.model.gated,
                        fit: rec.status,
                        explanation: rec.explanation,
                        recommended,
                    }
                })
                .collect::<Vec<_>>();

        if models.is_empty() {
            self.selected_model_index = 0;
            self.model_details_scroll = 0;
            self.models = models;
            return;
        }

        let target_model = prefer_model_id
            .map(str::to_owned)
            .or(previous_selection)
            .unwrap_or_else(|| models[0].id.clone());

        self.selected_model_index = models
            .iter()
            .position(|model| model.id == target_model)
            .unwrap_or(0);
        self.model_details_scroll = 0;

        self.models = std::mem::take(&mut models);
    }
}

fn parse_port(value: &str) -> Option<u16> {
    let parsed = value.trim().parse::<u16>().ok()?;
    if parsed == 0 { None } else { Some(parsed) }
}

fn resolve_backend_index(
    default_backend: &str,
    preferred_backend: PreferredBackend,
    choices: &[BackendChoice],
) -> usize {
    if let Some(index) = choices
        .iter()
        .position(|choice| choice.id.eq_ignore_ascii_case(default_backend))
    {
        return index;
    }

    if let Some(preferred) = preferred_backend.as_str()
        && let Some(index) = choices.iter().position(|choice| choice.id == preferred)
    {
        return index;
    }

    0
}
