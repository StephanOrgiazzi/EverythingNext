#[cfg(windows)]
mod bundled_engine;
mod model;
#[cfg(windows)]
mod sdk;
mod windows_name;

#[cfg(windows)]
use std::cell::RefCell;
#[cfg(windows)]
use std::cmp::Ordering;
#[cfg(windows)]
use std::collections::VecDeque;
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;

pub use model::*;
pub use windows_name::{validate_windows_name, WindowsNameError};

pub const MAX_CONCURRENT_THUMBNAIL_LOADS: usize = 16;

#[cfg(windows)]
const EVERYTHING3_ERROR_IPC_PIPE_NOT_FOUND: u32 = 0xE000_0002;
#[cfg(windows)]
const EVERYTHING3_ERROR_DISCONNECTED: u32 = 0xE000_0003;
#[cfg(windows)]
const EVERYTHING3_ERROR_SHUTDOWN: u32 = 0xE000_000C;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Everything SDK3 was not found. Install Everything3_x64.dll with scripts/install-everything-sdk.ps1 or set EVERYTHING_SDK3_DLL.")]
    SdkNotFound,
    #[error("Unable to load Everything SDK3: {0}")]
    SdkLoad(String),
    #[error("The bundled Everything 1.5 runtime was not found. Run scripts/install-everything-runtime.ps1 or set EVERYTHING_ENGINE_EXE.")]
    EngineNotFound,
    #[error("Everything is already running. Exit Everything before starting Everything Next.")]
    DefaultInstanceInUse,
    #[error("Invalid Everything instance name: {0}")]
    InvalidInstance(String),
    #[error("Unable to prepare the bundled Everything engine: {0}")]
    EngineSetup(String),
    #[error("Unable to start the bundled Everything engine: {0}")]
    EngineStart(String),
    #[error("Unable to connect Everything SDK3 to {instance} (code 0x{code:08X}). Make sure Everything 1.5 is running in that instance.")]
    ConnectionFailed { instance: String, code: u32 },
    #[error("Unsupported Everything version: {0}. Everything Next requires Everything 1.5.")]
    UnsupportedEverythingVersion(String),
    #[error("SDK3 call {operation} failed (code 0x{code:08X}).")]
    SdkCall { operation: &'static str, code: u32 },
    #[error("Everything Next requires Windows to access the Everything engine.")]
    UnsupportedPlatform,
    #[error("Invalid selection: {0}")]
    InvalidSelection(String),
    #[error("No indexed volume is ready yet.")]
    IndexUnavailable,
}

impl EngineError {
    #[cfg(windows)]
    fn is_reconnectable(&self) -> bool {
        let code = match self {
            Self::ConnectionFailed { code, .. } | Self::SdkCall { code, .. } => *code,
            Self::SdkNotFound
            | Self::SdkLoad(_)
            | Self::EngineNotFound
            | Self::DefaultInstanceInUse
            | Self::InvalidInstance(_)
            | Self::EngineSetup(_)
            | Self::EngineStart(_)
            | Self::UnsupportedEverythingVersion(_)
            | Self::UnsupportedPlatform
            | Self::InvalidSelection(_)
            | Self::IndexUnavailable => return false,
        };
        matches!(
            code,
            EVERYTHING3_ERROR_IPC_PIPE_NOT_FOUND
                | EVERYTHING3_ERROR_DISCONNECTED
                | EVERYTHING3_ERROR_SHUTDOWN
        )
    }
}

pub struct EverythingEngine {
    #[cfg(windows)]
    volumes: Vec<VolumeRuntime>,
    #[cfg(windows)]
    startup_errors: Vec<String>,
}

#[cfg(windows)]
struct VolumeRuntime {
    root: String,
    instance_name: String,
    sdk: RefCell<Option<sdk::EverythingSdk>>,
    dll_path: Option<PathBuf>,
    managed_engine: bundled_engine::ManagedEngine,
}

impl EverythingEngine {
    pub fn new() -> Result<Self, EngineError> {
        #[cfg(windows)]
        {
            Self::with_dll_path(None)
        }
        #[cfg(not(windows))]
        {
            Err(EngineError::UnsupportedPlatform)
        }
    }

    pub fn from_dll_path(_path: impl AsRef<Path>) -> Result<Self, EngineError> {
        #[cfg(windows)]
        {
            Self::with_dll_path(Some(_path.as_ref().to_path_buf()))
        }
        #[cfg(not(windows))]
        {
            Err(EngineError::UnsupportedPlatform)
        }
    }

    pub fn status(&self) -> EngineStatus {
        #[cfg(windows)]
        {
            let statuses = self
                .volumes
                .iter()
                .map(|volume| (volume.root.as_str(), volume.status()))
                .collect::<Vec<_>>();
            let ready = statuses
                .iter()
                .filter(|(_, status)| status.available)
                .map(|(root, _)| *root)
                .collect::<Vec<_>>();
            let total = self.volumes.len().saturating_add(self.startup_errors.len());
            let version = statuses
                .iter()
                .find_map(|(_, status)| status.version.clone());
            let message = match ready.len() {
                0 => statuses
                    .iter()
                    .map(|(root, status)| format!("{root} {}", status.message))
                    .chain(self.startup_errors.iter().cloned())
                    .collect::<Vec<_>>()
                    .join(" · "),
                count if count == total => {
                    format!("All {total} indexed volumes are ready.")
                }
                count => format!(
                    "{count}/{total} indexed volumes ready: {}",
                    ready.join(", ")
                ),
            };
            EngineStatus {
                available: !ready.is_empty(),
                indexing: ready.len() < total,
                ready_volumes: u32::try_from(ready.len()).unwrap_or(u32::MAX),
                total_volumes: u32::try_from(total).unwrap_or(u32::MAX),
                message,
                version,
            }
        }
        #[cfg(not(windows))]
        {
            EngineStatus {
                available: false,
                indexing: false,
                ready_volumes: 0,
                total_volumes: 0,
                message: EngineError::UnsupportedPlatform.to_string(),
                version: None,
            }
        }
    }

    pub fn query(&mut self, _request: QueryRequest) -> Result<SearchPage, EngineError> {
        #[cfg(windows)]
        {
            self.query_ready_volumes(_request)
        }
        #[cfg(not(windows))]
        {
            Err(EngineError::UnsupportedPlatform)
        }
    }

    pub fn resolve_selection_cancellable<F>(
        &mut self,
        _request: SelectionRequest,
        _max_items: usize,
        mut _is_cancelled: F,
    ) -> Result<Vec<String>, EngineError>
    where
        F: FnMut() -> bool,
    {
        #[cfg(windows)]
        {
            let ranges = normalize_selection_ranges(_request.ranges);
            let requested = ranges.iter().map(|range| range.len()).sum::<u64>();
            if requested > u64::try_from(_max_items).unwrap_or(u64::MAX) {
                return Err(EngineError::InvalidSelection(format!(
                    "This operation is limited to {_max_items} items at a time"
                )));
            }

            let mut paths = Vec::with_capacity(
                usize::try_from(requested).expect("validated selection count fits in usize"),
            );
            for range in ranges {
                let mut offset = range.start;
                while offset <= range.end {
                    if _is_cancelled() {
                        return Err(EngineError::InvalidSelection(
                            "The search changed while resolving the selection".to_string(),
                        ));
                    }
                    let remaining = range.end.saturating_sub(offset).saturating_add(1);
                    let page = self.query_ready_volumes(QueryRequest {
                        query: _request.query.clone(),
                        offset,
                        limit: remaining.min(256),
                        sort: _request.sort,
                        request_id: _request.request_id,
                    })?;
                    if page.items.is_empty() {
                        return Err(EngineError::InvalidSelection(
                            "The selection no longer matches the current results".to_string(),
                        ));
                    }
                    let received =
                        u32::try_from(page.items.len()).expect("a search page always fits in u32");
                    paths.extend(page.items.into_iter().map(|item| item.full_path));
                    offset = offset.saturating_add(received);
                }
            }
            Ok(paths)
        }
        #[cfg(not(windows))]
        {
            Err(EngineError::UnsupportedPlatform)
        }
    }

    #[cfg(windows)]
    fn with_dll_path(dll_path: Option<PathBuf>) -> Result<Self, EngineError> {
        let targets = bundled_engine::fixed_volumes()?;
        let mut volumes = Vec::with_capacity(targets.len());
        let mut startup_errors = Vec::new();
        for target in targets {
            match bundled_engine::ManagedEngine::start(&target) {
                Ok(managed_engine) => volumes.push(VolumeRuntime {
                    root: target.root,
                    instance_name: managed_engine.instance_name().to_string(),
                    sdk: RefCell::new(None),
                    dll_path: dll_path.clone(),
                    managed_engine,
                }),
                Err(error) => startup_errors.push(format!("{}: {error}", target.root)),
            }
        }
        if volumes.is_empty() {
            return Err(EngineError::EngineStart(startup_errors.join(" · ")));
        }
        Ok(Self {
            volumes,
            startup_errors,
        })
    }

    #[cfg(windows)]
    fn query_ready_volumes(&mut self, request: QueryRequest) -> Result<SearchPage, EngineError> {
        let ready = self
            .volumes
            .iter()
            .enumerate()
            .filter(|(_, volume)| volume.status().available)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if ready.is_empty() {
            return Err(EngineError::IndexUnavailable);
        }
        if let [index] = ready.as_slice() {
            return self.volumes[*index].query(request);
        }

        let chunk_size = 256;
        let mut cursors = Vec::with_capacity(ready.len());
        let mut total = 0_u64;
        for volume_index in ready {
            let page = self.volumes[volume_index].query(QueryRequest {
                offset: 0,
                limit: chunk_size,
                ..request.clone()
            })?;
            total = total.saturating_add(u64::from(page.total));
            if page.total > 0 {
                let next_offset =
                    u32::try_from(page.items.len()).expect("a search page always fits in u32");
                cursors.push(SourceCursor {
                    volume_index,
                    next_offset,
                    total: page.total,
                    items: page.items.into(),
                });
            }
        }

        let requested_limit = request.limit.clamp(1, 4096);
        let target = u64::from(request.offset).saturating_add(u64::from(requested_limit));
        let mut emitted = 0_u64;
        let mut items = Vec::with_capacity(requested_limit as usize);
        while emitted < target {
            let Some(cursor_index) = next_cursor(&cursors, request.sort) else {
                break;
            };
            let item = cursors[cursor_index]
                .items
                .pop_front()
                .expect("the selected source has a front item");
            if emitted >= u64::from(request.offset) {
                items.push(item);
            }
            emitted = emitted.saturating_add(1);

            if cursors[cursor_index].items.is_empty()
                && cursors[cursor_index].next_offset < cursors[cursor_index].total
            {
                let source_offset = cursors[cursor_index].next_offset;
                let volume_index = cursors[cursor_index].volume_index;
                let page = self.volumes[volume_index].query(QueryRequest {
                    offset: source_offset,
                    limit: chunk_size,
                    ..request.clone()
                })?;
                if page.items.is_empty() {
                    cursors[cursor_index].next_offset = cursors[cursor_index].total;
                } else {
                    cursors[cursor_index].next_offset = source_offset.saturating_add(
                        u32::try_from(page.items.len()).expect("a search page always fits in u32"),
                    );
                    cursors[cursor_index].items = page.items.into();
                }
            }
        }

        Ok(SearchPage {
            request_id: request.request_id,
            offset: request.offset,
            total: u32::try_from(total).unwrap_or(u32::MAX),
            items,
        })
    }
}

impl Drop for EverythingEngine {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            for volume in &mut self.volumes {
                volume.sdk.replace(None);
                volume.managed_engine.stop();
            }
        }
    }
}

#[cfg(windows)]
impl VolumeRuntime {
    fn status(&self) -> EngineStatus {
        match self.ensure_connected() {
            Ok(()) => self
                .sdk
                .borrow()
                .as_ref()
                .expect("connected SDK3 is missing")
                .status(),
            Err(error) => EngineStatus {
                available: false,
                indexing: true,
                ready_volumes: 0,
                total_volumes: 1,
                message: error.to_string(),
                version: None,
            },
        }
    }

    fn ensure_connected(&self) -> Result<(), EngineError> {
        if self.sdk.borrow().is_some() {
            return Ok(());
        }
        let result = match self.dll_path.as_deref() {
            Some(path) => sdk::EverythingSdk::load_from(path, &self.instance_name),
            None => sdk::EverythingSdk::load(&self.instance_name),
        };
        result.map(|sdk| {
            self.sdk.replace(Some(sdk));
        })
    }

    fn query(&mut self, request: QueryRequest) -> Result<SearchPage, EngineError> {
        let first_result = self.query_once(request.clone());
        if let Err(error) = &first_result {
            if error.is_reconnectable() {
                self.sdk.replace(None);
                let retry_result = self.query_once(request);
                if retry_result
                    .as_ref()
                    .is_err_and(EngineError::is_reconnectable)
                {
                    self.sdk.replace(None);
                }
                return retry_result;
            }
        }
        first_result
    }

    fn query_once(&self, request: QueryRequest) -> Result<SearchPage, EngineError> {
        self.ensure_connected()?;
        self.sdk
            .borrow_mut()
            .as_mut()
            .expect("connected SDK3 is missing")
            .query(request)
    }
}

#[cfg(windows)]
struct SourceCursor {
    volume_index: usize,
    next_offset: u32,
    total: u32,
    items: VecDeque<SearchResult>,
}

#[cfg(windows)]
fn next_cursor(cursors: &[SourceCursor], sort: SortSpec) -> Option<usize> {
    cursors
        .iter()
        .enumerate()
        .filter(|(_, cursor)| !cursor.items.is_empty())
        .min_by(|(_, left), (_, right)| {
            compare_results(
                left.items.front().expect("non-empty source has a front"),
                right.items.front().expect("non-empty source has a front"),
                sort,
            )
        })
        .map(|(index, _)| index)
}

#[cfg(windows)]
fn compare_results(left: &SearchResult, right: &SearchResult, sort: SortSpec) -> Ordering {
    let order = match sort.column {
        SortColumn::Name => compare_text(&left.name, &right.name)
            .then_with(|| compare_text(&left.parent_path, &right.parent_path)),
        SortColumn::Path => compare_text(&left.parent_path, &right.parent_path)
            .then_with(|| compare_text(&left.name, &right.name)),
        SortColumn::Extension => compare_text(extension(&left.name), extension(&right.name))
            .then_with(|| compare_text(&left.name, &right.name))
            .then_with(|| compare_text(&left.parent_path, &right.parent_path)),
        SortColumn::Size => left
            .size
            .cmp(&right.size)
            .then_with(|| compare_text(&left.name, &right.name))
            .then_with(|| compare_text(&left.parent_path, &right.parent_path)),
        SortColumn::Modified => left
            .modified_unix
            .cmp(&right.modified_unix)
            .then_with(|| compare_text(&left.name, &right.name))
            .then_with(|| compare_text(&left.parent_path, &right.parent_path)),
    };
    match sort.direction {
        SortDirection::Ascending => order,
        SortDirection::Descending => order.reverse(),
    }
}

#[cfg(windows)]
fn compare_text(left: &str, right: &str) -> Ordering {
    left.to_lowercase()
        .cmp(&right.to_lowercase())
        .then_with(|| left.cmp(right))
}

#[cfg(windows)]
fn extension(name: &str) -> &str {
    name.rsplit_once('.')
        .filter(|(stem, extension)| !stem.is_empty() && !extension.is_empty())
        .map_or("", |(_, extension)| extension)
}

#[cfg(windows)]
fn normalize_selection_ranges(mut ranges: Vec<SelectionRange>) -> Vec<SelectionRange> {
    for range in &mut ranges {
        *range = SelectionRange::new(range.start, range.end);
    }
    ranges.sort_unstable_by_key(|range| range.start);
    let mut normalized: Vec<SelectionRange> = Vec::with_capacity(ranges.len());
    for range in ranges {
        if let Some(last) = normalized.last_mut() {
            if range.start <= last.end.saturating_add(1) {
                last.end = last.end.max(range.end);
                continue;
            }
        }
        normalized.push(range);
    }
    normalized
}

#[cfg(all(test, windows))]
mod federation_tests {
    use super::*;

    fn result(path: &str) -> SearchResult {
        let (parent_path, name) = path.rsplit_once('\\').unwrap_or(("", path));
        SearchResult {
            id: path.to_string(),
            name: name.to_string(),
            parent_path: parent_path.to_string(),
            full_path: path.to_string(),
            size: None,
            modified_unix: None,
            is_dir: false,
        }
    }

    #[test]
    fn chooses_the_next_result_across_sorted_volumes() {
        let cursors = vec![
            SourceCursor {
                volume_index: 0,
                next_offset: 1,
                total: 1,
                items: VecDeque::from([result(r"C:\beta.txt")]),
            },
            SourceCursor {
                volume_index: 1,
                next_offset: 1,
                total: 1,
                items: VecDeque::from([result(r"D:\Alpha.txt")]),
            },
        ];

        assert_eq!(next_cursor(&cursors, SortSpec::default()), Some(1));
    }

    #[test]
    fn normalizes_overlapping_selection_ranges() {
        let ranges = normalize_selection_ranges(vec![
            SelectionRange::new(20, 25),
            SelectionRange::new(4, 8),
            SelectionRange::new(9, 12),
            SelectionRange::new(24, 30),
        ]);

        assert_eq!(
            ranges,
            vec![SelectionRange::new(4, 12), SelectionRange::new(20, 30)]
        );
    }
}
