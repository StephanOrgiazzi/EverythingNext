#[cfg(windows)]
mod bundled_engine;
mod model;
#[cfg(windows)]
mod sdk;
mod windows_name;

#[cfg(windows)]
use std::cell::RefCell;
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;

pub use model::*;
pub use windows_name::{validate_windows_name, WindowsNameError};

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
    #[error("Everything is already running. Exit Everything before starting Everything Modern.")]
    DefaultInstanceInUse,
    #[error("Invalid Everything instance name: {0}")]
    InvalidInstance(String),
    #[error("Unable to prepare the bundled Everything engine: {0}")]
    EngineSetup(String),
    #[error("Unable to start the bundled Everything engine: {0}")]
    EngineStart(String),
    #[error("Unable to connect Everything SDK3 to {instance} (code 0x{code:08X}). Make sure Everything 1.5 is running in that instance.")]
    ConnectionFailed { instance: String, code: u32 },
    #[error("Unsupported Everything version: {0}. Everything Modern requires Everything 1.5.")]
    UnsupportedEverythingVersion(String),
    #[error("SDK3 call {operation} failed (code 0x{code:08X}).")]
    SdkCall { operation: &'static str, code: u32 },
    #[error("Everything Modern requires Windows to access the Everything engine.")]
    UnsupportedPlatform,
    #[error("Invalid selection: {0}")]
    InvalidSelection(String),
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
    #[cfg(windows)]
    sdk: RefCell<Option<sdk::EverythingSdk>>,
    #[cfg(windows)]
    dll_path: Option<PathBuf>,
    #[cfg(windows)]
    managed_engine: Option<bundled_engine::ManagedEngine>,
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
            match self.ensure_connected() {
                Ok(()) => self
                    .sdk
                    .borrow()
                    .as_ref()
                    .expect("connected SDK3 is missing")
                    .status(),
                Err(error) => EngineStatus {
                    available: false,
                    message: error.to_string(),
                    version: None,
                },
            }
        }
        #[cfg(not(windows))]
        {
            EngineStatus {
                available: false,
                message: EngineError::UnsupportedPlatform.to_string(),
                version: None,
            }
        }
    }

    pub fn query(&mut self, _request: QueryRequest) -> Result<SearchPage, EngineError> {
        #[cfg(windows)]
        {
            let first_result = self.query_once(_request.clone());
            if let Err(error) = &first_result {
                if error.is_reconnectable() {
                    self.invalidate_connection();
                    let retry_result = self.query_once(_request);
                    if let Err(retry_error) = &retry_result {
                        if retry_error.is_reconnectable() {
                            self.invalidate_connection();
                        }
                    }
                    return retry_result;
                }
            }
            first_result
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
        _is_cancelled: F,
    ) -> Result<Vec<String>, EngineError>
    where
        F: FnMut() -> bool,
    {
        #[cfg(windows)]
        {
            self.ensure_connected()?;
            let result = {
                let mut sdk = self.sdk.borrow_mut();
                sdk.as_mut()
                    .expect("connected SDK3 is missing")
                    .resolve_selection_cancellable(_request, _max_items, _is_cancelled)
            };
            if let Err(error) = &result {
                if error.is_reconnectable() {
                    self.invalidate_connection();
                }
            }
            result
        }
        #[cfg(not(windows))]
        {
            Err(EngineError::UnsupportedPlatform)
        }
    }

    #[cfg(windows)]
    fn with_dll_path(dll_path: Option<PathBuf>) -> Result<Self, EngineError> {
        let managed_engine = Some(bundled_engine::ManagedEngine::start()?);
        let engine = Self {
            sdk: RefCell::new(None),
            dll_path,
            managed_engine,
        };
        match engine.ensure_connected() {
            Ok(()) => Ok(engine),
            Err(error) if error.is_reconnectable() => Ok(engine),
            Err(error) => Err(error),
        }
    }

    #[cfg(windows)]
    fn ensure_connected(&self) -> Result<(), EngineError> {
        if self.sdk.borrow().is_some() {
            return Ok(());
        }

        let result = self
            .dll_path
            .as_deref()
            .map(sdk::EverythingSdk::load_from)
            .unwrap_or_else(sdk::EverythingSdk::load);

        match result {
            Ok(sdk) => {
                self.sdk.replace(Some(sdk));
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    #[cfg(windows)]
    fn query_once(&self, request: QueryRequest) -> Result<SearchPage, EngineError> {
        self.ensure_connected()?;
        self.sdk
            .borrow_mut()
            .as_mut()
            .expect("connected SDK3 is missing")
            .query(request)
    }

    #[cfg(windows)]
    fn invalidate_connection(&self) {
        self.sdk.replace(None);
    }

    #[cfg(windows)]
    fn disconnect_sdk(&self) {
        self.sdk.replace(None);
    }
}

impl Drop for EverythingEngine {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            self.disconnect_sdk();
            if let Some(mut managed_engine) = self.managed_engine.take() {
                managed_engine.stop();
            }
        }
    }
}
