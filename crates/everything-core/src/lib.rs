mod model;
#[cfg(windows)]
mod sdk;

#[cfg(windows)]
use std::cell::RefCell;
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;

pub use model::*;

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
            _ => return false,
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
}

impl EverythingEngine {
    /// Charge Everything SDK3 depuis le bundle de développement ou depuis
    /// `EVERYTHING_SDK3_DLL`, puis tente de se connecter à l’instance Everything 1.5
    /// configurée par `EVERYTHING_INSTANCE` (instance principale par défaut).
    /// Un échec IPC transitoire est toléré afin que les appels suivants puissent
    /// retenter la connexion sans redémarrer l’application.
    pub fn new() -> Result<Self, EngineError> {
        #[cfg(windows)]
        {
            return Self::with_dll_path(None);
        }
        #[cfg(not(windows))]
        {
            Err(EngineError::UnsupportedPlatform)
        }
    }

    /// Charge explicitement la DLL SDK3. Cette API garde la crate indépendante
    /// de Tauri tout en permettant au shell desktop de fournir le chemin de la
    /// ressource `Everything3_x64.dll` du bundle installé.
    pub fn from_dll_path(path: impl AsRef<Path>) -> Result<Self, EngineError> {
        #[cfg(windows)]
        {
            return Self::with_dll_path(Some(path.as_ref().to_path_buf()));
        }
        #[cfg(not(windows))]
        {
            let _ = path;
            Err(EngineError::UnsupportedPlatform)
        }
    }

    pub fn status(&self) -> EngineStatus {
        #[cfg(windows)]
        {
            return match self.ensure_connected() {
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
            };
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

    pub fn query(&mut self, request: QueryRequest) -> Result<SearchPage, EngineError> {
        #[cfg(windows)]
        {
            let first_result = self.query_once(request.clone());
            if let Err(error) = &first_result {
                if error.is_reconnectable() {
                    self.invalidate_connection();
                    let retry_result = self.query_once(request);
                    if let Err(retry_error) = &retry_result {
                        if retry_error.is_reconnectable() {
                            self.invalidate_connection();
                        }
                    }
                    return retry_result;
                }
            }
            return first_result;
        }
        #[cfg(not(windows))]
        {
            let _ = request;
            Err(EngineError::UnsupportedPlatform)
        }
    }

    pub fn resolve_selection_cancellable<F>(
        &mut self,
        request: SelectionRequest,
        max_items: usize,
        is_cancelled: F,
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
                    .resolve_selection_cancellable(request, max_items, is_cancelled)
            };
            if let Err(error) = &result {
                if error.is_reconnectable() {
                    self.invalidate_connection();
                }
            }
            return result;
        }
        #[cfg(not(windows))]
        {
            let _ = (request, max_items, is_cancelled);
            Err(EngineError::UnsupportedPlatform)
        }
    }

    #[cfg(windows)]
    fn with_dll_path(dll_path: Option<PathBuf>) -> Result<Self, EngineError> {
        let engine = Self {
            sdk: RefCell::new(None),
            dll_path,
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
}
