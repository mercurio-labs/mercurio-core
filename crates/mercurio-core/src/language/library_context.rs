use crate::ir::KirDocument;
use crate::paths::{default_kernel_library_path, default_sysml_library_path};

#[derive(Debug, Clone)]
pub enum BaselineLibrary {
    Empty,
    Kernel,
    Sysml,
    Custom(KirDocument),
}

impl BaselineLibrary {
    pub fn load(&self) -> Result<KirDocument, crate::ir::KirError> {
        match self {
            Self::Empty => Ok(KirDocument {
                metadata: Default::default(),
                elements: Vec::new(),
            }),
            Self::Kernel => KirDocument::from_path(&default_kernel_library_path()),
            Self::Sysml => KirDocument::from_path(&default_sysml_library_path()),
            Self::Custom(document) => Ok(document.clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LibraryContext {
    pub baseline: BaselineLibrary,
    pub document: KirDocument,
}

impl LibraryContext {
    pub fn empty() -> Self {
        Self::from_document(
            BaselineLibrary::Empty,
            KirDocument {
                metadata: Default::default(),
                elements: Vec::new(),
            },
        )
    }

    pub fn from_document(baseline: BaselineLibrary, document: KirDocument) -> Self {
        Self { baseline, document }
    }
}
