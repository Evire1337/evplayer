//! The native build has no song splitting ("Дорожки" is left out: it was slow and did not work
//! as it should). media_server.rs, shared with the Tauri build, still names the splitter's
//! type; this stand-in has nothing to serve, so the shared file compiles unchanged and no ONNX
//! Runtime is linked in.

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Stems;

impl Stems {
    pub fn new(_data: &Path) -> Arc<Self> {
        Arc::new(Self)
    }

    pub fn path_of(&self, _key: &str, _part: &str) -> Option<PathBuf> {
        None
    }
}
