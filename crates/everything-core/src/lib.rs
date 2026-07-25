mod model;
#[cfg(windows)]
mod sdk;

use std::path::Path;

pub use model::*;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Everything SDK3 introuvable. Installez Everything3_x64.dll avec scripts/install-everything-sdk.ps1 ou définissez EVERYTHING_SDK3_DLL.")]
    SdkNotFound,
    #[error("Impossible de charger Everything SDK3 : {0}")]
    SdkLoad(String),
    #[error("Impossible de connecter Everything SDK3 à {instance} (code 0x{code:08X}). Vérifiez qu’Everything 1.5 est lancé dans cette instance.")]
    ConnectionFailed { instance: String, code: u32 },
    #[error("Version Everything non prise en charge : {0}. Everything Modern nécessite Everything 1.5.")]
    UnsupportedEverythingVersion(String),
    #[error("Échec de l’appel SDK3 {operation} (code 0x{code:08X}).")]
    SdkCall { operation: &'static str, code: u32 },
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
    /// Charge Everything SDK3 depuis le bundle de développement ou depuis
    /// `EVERYTHING_SDK3_DLL`, puis se connecte à l’instance Everything 1.5
    /// configurée par `EVERYTHING_INSTANCE` (instance principale par défaut).
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

    /// Charge explicitement la DLL SDK3. Cette API garde la crate indépendante
    /// de Tauri tout en permettant au shell desktop de fournir le chemin de la
    /// ressource `Everything3_x64.dll` du bundle installé.
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
