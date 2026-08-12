#![cfg(windows)]

mod bundled_engine;
mod sdk;

use everything_core::{EngineStatus, QueryRequest, SearchPage, SelectionRange, SelectionRequest};
use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;

const EVERYTHING3_ERROR_IPC_PIPE_NOT_FOUND: u32 = 0xE000_0002;
const EVERYTHING3_ERROR_DISCONNECTED: u32 = 0xE000_0003;
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
    #[error("Invalid selection: {0}")]
    InvalidSelection(String),
}

impl EngineError {
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
            | Self::InvalidSelection(_) => return false,
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
    sdk: RefCell<Option<sdk::EverythingSdk>>,
    instance_name: String,
    dll_path: Option<PathBuf>,
    managed_engine: Option<bundled_engine::ManagedEngine>,
}

impl EverythingEngine {
    pub fn new() -> Result<Self, EngineError> {
        Self::with_dll_path(None)
    }

    pub fn from_dll_path(path: impl AsRef<Path>) -> Result<Self, EngineError> {
        Self::with_dll_path(Some(path.as_ref().to_path_buf()))
    }

    #[must_use]
    pub fn status(&self) -> EngineStatus {
        match self.ensure_connected() {
            Ok(()) => self.sdk.borrow().as_ref().map_or_else(
                || EngineStatus {
                    available: false,
                    indexing: false,
                    ready_volumes: 0,
                    total_volumes: 0,
                    message: "Everything SDK3 connection state is inconsistent".into(),
                    version: None,
                },
                sdk::EverythingSdk::status,
            ),
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

    pub fn query(&mut self, request: &QueryRequest) -> Result<SearchPage, EngineError> {
        let first_result = self.query_once(request);
        if first_result
            .as_ref()
            .is_err_and(EngineError::is_reconnectable)
        {
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
        first_result
    }

    pub fn resolve_selection_cancellable<F>(
        &mut self,
        request: SelectionRequest,
        max_items: usize,
        mut is_cancelled: F,
    ) -> Result<Vec<String>, EngineError>
    where
        F: FnMut() -> bool,
    {
        let ranges = normalize_selection_ranges(request.ranges);
        let requested = ranges
            .iter()
            .map(|range| range.inclusive_len())
            .sum::<u64>();
        if requested > u64::try_from(max_items).unwrap_or(u64::MAX) {
            return Err(EngineError::InvalidSelection(format!(
                "This operation is limited to {max_items} items at a time"
            )));
        }

        let capacity = usize::try_from(requested).map_err(|_| {
            EngineError::InvalidSelection("The selection is too large for this platform".into())
        })?;
        let mut paths = Vec::with_capacity(capacity);
        for range in ranges {
            let mut offset = range.start;
            while offset <= range.end {
                if is_cancelled() {
                    return Err(EngineError::InvalidSelection(
                        "The search changed while resolving the selection".to_string(),
                    ));
                }
                let remaining = range.end.saturating_sub(offset).saturating_add(1);
                let page = self.query(&QueryRequest {
                    query: request.query.clone(),
                    offset,
                    limit: remaining.min(256),
                    sort: request.sort,
                    request_id: request.request_id,
                })?;
                if page.items.is_empty() {
                    return Err(EngineError::InvalidSelection(
                        "The selection no longer matches the current results".to_string(),
                    ));
                }
                let received = u32::try_from(page.items.len()).map_err(|_| {
                    EngineError::InvalidSelection("The result page is too large".into())
                })?;
                paths.extend(page.items.into_iter().map(|item| item.full_path));
                offset = offset.saturating_add(received);
            }
        }
        Ok(paths)
    }

    fn with_dll_path(dll_path: Option<PathBuf>) -> Result<Self, EngineError> {
        let managed_engine = bundled_engine::ManagedEngine::start()?;
        let instance_name = managed_engine.instance_name().to_string();
        Ok(Self {
            sdk: RefCell::new(None),
            instance_name,
            dll_path,
            managed_engine: Some(managed_engine),
        })
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

    fn query_once(&self, request: &QueryRequest) -> Result<SearchPage, EngineError> {
        self.ensure_connected()?;
        self.sdk
            .borrow_mut()
            .as_mut()
            .expect("connected SDK3 is missing")
            .query(request)
    }
}

impl Drop for EverythingEngine {
    fn drop(&mut self) {
        self.sdk.replace(None);
        if let Some(mut managed_engine) = self.managed_engine.take() {
            managed_engine.stop();
        }
    }
}

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

#[cfg(test)]
mod selection_tests {
    use super::*;

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
