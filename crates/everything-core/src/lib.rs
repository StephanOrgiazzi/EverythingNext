mod model;
#[cfg(windows)]
mod sdk;

use std::path::Path;

pub use model::*;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Everything SDK introuvable. Installez Everything64.dll avec scripts/install-everything-sdk.ps1 ou définissez EVERYTHING_SDK_DLL.")]
    SdkNotFound,
    #[error("Impossible de charger le SDK Everything : {0}")]
    SdkLoad(String),
    #[error("Everything ne répond pas via IPC (code {0}). Vérifiez que Everything est lancé.")]
    QueryFailed(u32),
    #[error("Everything Modern nécessite Windows pour accéder au moteur Everything.")]
    UnsupportedPlatform,
    #[error("Sélection invalide : {0}")]
    InvalidSelection(String),
}

pub struct EverythingEngine {
    #[cfg(windows)]
    sdk: sdk::EverythingSdk,
}

impl EverythingEngine {
    /// Charge le SDK depuis les emplacements de développement usuels ou la variable
    /// `EVERYTHING_SDK_DLL`. L'application Tauri utilise plutôt `from_dll_path` afin
    /// de résoudre explicitement la ressource du bundle installé.
    pub fn new() -> Result<Self, EngineError> {
        #[cfg(windows)]
        {
            return Ok(Self {
                sdk: sdk::EverythingSdk::load()?,
            });
        }
        #[cfg(not(windows))]
        {
            Err(EngineError::UnsupportedPlatform)
        }
    }

    /// Charge explicitement une DLL Everything SDK. Cette API garde la crate
    /// indépendante de Tauri tout en permettant au shell desktop de lui fournir
    /// le chemin `$RESOURCE/Everything64.dll`.
    pub fn from_dll_path(path: impl AsRef<Path>) -> Result<Self, EngineError> {
        #[cfg(windows)]
        {
            return Ok(Self {
                sdk: sdk::EverythingSdk::load_from(path.as_ref())?,
            });
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
            self.sdk.status()
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
            return self.sdk.query(request);
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
            return self
                .sdk
                .resolve_selection_cancellable(request, max_items, is_cancelled);
        }
        #[cfg(not(windows))]
        {
            let _ = (request, max_items, is_cancelled);
            Err(EngineError::UnsupportedPlatform)
        }
    }
}
