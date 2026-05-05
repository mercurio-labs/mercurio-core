use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ir::{KirDocument, KirError};
use crate::paths::default_stdlib_path;
use crate::source_set::{SourceDocument, compile_source_documents};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BaselineLibraryConfig {
    #[serde(default = "default_baseline_library_id")]
    pub id: String,
    #[serde(default)]
    pub provider: LibraryProviderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LibraryProviderConfig {
    #[default]
    BundledStdlib,
    #[serde(alias = "local_kir_file")]
    PrecompiledKirArtifact {
        path: String,
    },
    SysmlDirectory {
        path: String,
    },
    KparFile {
        path: String,
    },
    PackageSetDirectory {
        path: String,
        entry: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LibraryCacheMetadata {
    pub source_kind: String,
    pub source_identity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_digest: Option<String>,
    pub importer_version: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedLibraryArtifact {
    pub library_id: String,
    pub source_kind: String,
    pub source_path: Option<PathBuf>,
    pub cache_metadata: Option<LibraryCacheMetadata>,
    pub document: KirDocument,
}

#[derive(Debug, Clone)]
pub struct LibrarySourceFingerprint {
    pub library_id: String,
    pub source_kind: String,
    pub source_path: Option<PathBuf>,
    pub cache_metadata: LibraryCacheMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KparPackageBuild {
    pub name: String,
    pub version: Option<String>,
    pub sources: Vec<KparPackageSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KparPackageSource {
    pub path: String,
    pub content: String,
}

impl Default for BaselineLibraryConfig {
    fn default() -> Self {
        Self::bundled_stdlib()
    }
}

impl BaselineLibraryConfig {
    pub fn bundled_stdlib() -> Self {
        Self {
            id: "stdlib".to_string(),
            provider: LibraryProviderConfig::BundledStdlib,
        }
    }

    pub fn resolve(&self) -> Result<ResolvedLibraryArtifact, KirError> {
        self.provider.resolve(&self.id)
    }

    pub fn resolve_from(&self, base_dir: &Path) -> Result<ResolvedLibraryArtifact, KirError> {
        self.provider.resolve_from(&self.id, Some(base_dir))
    }
}

impl LibraryProviderConfig {
    pub fn resolve(&self, library_id: &str) -> Result<ResolvedLibraryArtifact, KirError> {
        self.resolve_with_context(library_id, None, None)
    }

    pub fn resolve_from(
        &self,
        library_id: &str,
        base_dir: Option<&Path>,
    ) -> Result<ResolvedLibraryArtifact, KirError> {
        self.resolve_with_context(library_id, base_dir, None)
    }

    pub fn resolve_with_context(
        &self,
        library_id: &str,
        base_dir: Option<&Path>,
        library_context: Option<&KirDocument>,
    ) -> Result<ResolvedLibraryArtifact, KirError> {
        match self {
            Self::BundledStdlib => {
                let fingerprint = self.source_fingerprint(library_id, base_dir)?;
                let source_path = fingerprint
                    .source_path
                    .clone()
                    .unwrap_or_else(default_stdlib_path);
                let document = KirDocument::from_path(&source_path)?;

                Ok(ResolvedLibraryArtifact {
                    library_id: library_id.to_string(),
                    source_kind: fingerprint.source_kind,
                    source_path: Some(source_path),
                    cache_metadata: Some(fingerprint.cache_metadata),
                    document,
                })
            }
            Self::PrecompiledKirArtifact { path } => {
                let fingerprint = self.source_fingerprint(library_id, base_dir)?;
                let source_path = fingerprint
                    .source_path
                    .clone()
                    .unwrap_or_else(|| resolve_provider_path(path, base_dir));
                let document = KirDocument::from_path(&source_path)?;

                Ok(ResolvedLibraryArtifact {
                    library_id: library_id.to_string(),
                    source_kind: fingerprint.source_kind,
                    source_path: Some(source_path),
                    cache_metadata: Some(fingerprint.cache_metadata),
                    document,
                })
            }
            Self::SysmlDirectory { path } => {
                let fingerprint = self.source_fingerprint(library_id, base_dir)?;
                let source_path = fingerprint
                    .source_path
                    .clone()
                    .unwrap_or_else(|| resolve_provider_path(path, base_dir));
                let fallback_context = KirDocument::from_path(&default_stdlib_path())?;
                let context_document = library_context.unwrap_or(&fallback_context);
                let document = compile_sysml_directory(&source_path, context_document)?;

                Ok(ResolvedLibraryArtifact {
                    library_id: library_id.to_string(),
                    source_kind: fingerprint.source_kind,
                    source_path: Some(source_path.clone()),
                    cache_metadata: Some(fingerprint.cache_metadata),
                    document,
                })
            }
            Self::KparFile { path } => {
                let fingerprint = self.source_fingerprint(library_id, base_dir)?;
                let source_path = fingerprint
                    .source_path
                    .clone()
                    .unwrap_or_else(|| resolve_provider_path(path, base_dir));
                let fallback_context = KirDocument::from_path(&default_stdlib_path())?;
                let context_document = library_context.unwrap_or(&fallback_context);
                let (document, package_metadata) =
                    compile_kpar_file(&source_path, context_document)?;

                Ok(ResolvedLibraryArtifact {
                    library_id: library_id.to_string(),
                    source_kind: fingerprint.source_kind,
                    source_path: Some(source_path.clone()),
                    cache_metadata: Some(LibraryCacheMetadata {
                        source_version: package_metadata
                            .and_then(|metadata| metadata.version)
                            .or(fingerprint.cache_metadata.source_version.clone()),
                        ..fingerprint.cache_metadata
                    }),
                    document,
                })
            }
            Self::PackageSetDirectory { path, entry } => {
                let fingerprint = self.source_fingerprint(library_id, base_dir)?;
                let source_path = fingerprint
                    .source_path
                    .clone()
                    .unwrap_or_else(|| resolve_provider_path(path, base_dir));
                let fallback_context = KirDocument::from_path(&default_stdlib_path())?;
                let context_document = library_context.unwrap_or(&fallback_context);
                let (document, package_metadata) =
                    compile_kpar_package_set(&source_path, entry, context_document)?;

                Ok(ResolvedLibraryArtifact {
                    library_id: library_id.to_string(),
                    source_kind: fingerprint.source_kind,
                    source_path: Some(source_path.clone()),
                    cache_metadata: Some(LibraryCacheMetadata {
                        source_version: package_metadata.and_then(|metadata| metadata.version),
                        ..fingerprint.cache_metadata
                    }),
                    document,
                })
            }
        }
    }

    pub fn source_fingerprint(
        &self,
        library_id: &str,
        base_dir: Option<&Path>,
    ) -> Result<LibrarySourceFingerprint, KirError> {
        let importer_version = env!("CARGO_PKG_VERSION").to_string();
        match self {
            Self::BundledStdlib => {
                let source_path = default_stdlib_path();
                Ok(LibrarySourceFingerprint {
                    library_id: library_id.to_string(),
                    source_kind: "bundled_stdlib".to_string(),
                    source_path: Some(source_path.clone()),
                    cache_metadata: LibraryCacheMetadata {
                        source_kind: "bundled_stdlib".to_string(),
                        source_identity: source_path.display().to_string(),
                        source_version: None,
                        source_digest: Some(digest_file(&source_path)?),
                        importer_version,
                    },
                })
            }
            Self::PrecompiledKirArtifact { path } => {
                let source_path = resolve_provider_path(path, base_dir);
                Ok(LibrarySourceFingerprint {
                    library_id: library_id.to_string(),
                    source_kind: "precompiled_kir_artifact".to_string(),
                    source_path: Some(source_path.clone()),
                    cache_metadata: LibraryCacheMetadata {
                        source_kind: "precompiled_kir_artifact".to_string(),
                        source_identity: source_path.display().to_string(),
                        source_version: None,
                        source_digest: Some(digest_file(&source_path)?),
                        importer_version,
                    },
                })
            }
            Self::SysmlDirectory { path } => {
                let source_path = resolve_provider_path(path, base_dir);
                Ok(LibrarySourceFingerprint {
                    library_id: library_id.to_string(),
                    source_kind: "sysml_directory".to_string(),
                    source_path: Some(source_path.clone()),
                    cache_metadata: LibraryCacheMetadata {
                        source_kind: "sysml_directory".to_string(),
                        source_identity: source_path.display().to_string(),
                        source_version: None,
                        source_digest: Some(digest_sysml_directory(&source_path)?),
                        importer_version,
                    },
                })
            }
            Self::KparFile { path } => {
                let source_path = resolve_provider_path(path, base_dir);
                let (_, package_metadata) = collect_kpar_source_files(&source_path)?;
                Ok(LibrarySourceFingerprint {
                    library_id: library_id.to_string(),
                    source_kind: "kpar_file".to_string(),
                    source_path: Some(source_path.clone()),
                    cache_metadata: LibraryCacheMetadata {
                        source_kind: "kpar_file".to_string(),
                        source_identity: source_path.display().to_string(),
                        source_version: package_metadata.and_then(|metadata| metadata.version),
                        source_digest: Some(digest_file(&source_path)?),
                        importer_version,
                    },
                })
            }
            Self::PackageSetDirectory { path, entry } => {
                let source_path = resolve_provider_path(path, base_dir);
                let package_index = build_kpar_package_index(&source_path)?;
                let source_version = package_index
                    .resolve(entry, None)
                    .and_then(|entry_key| package_index.packages.get(&entry_key))
                    .and_then(|package| package.metadata.as_ref())
                    .and_then(|metadata| metadata.version.clone());
                Ok(LibrarySourceFingerprint {
                    library_id: library_id.to_string(),
                    source_kind: "package_set_directory".to_string(),
                    source_path: Some(source_path.clone()),
                    cache_metadata: LibraryCacheMetadata {
                        source_kind: "package_set_directory".to_string(),
                        source_identity: format!("{}#{}", source_path.display(), entry),
                        source_version,
                        source_digest: Some(digest_kpar_directory(&source_path)?),
                        importer_version,
                    },
                })
            }
        }
    }
}

fn resolve_provider_path(path: &str, base_dir: Option<&Path>) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_relative() {
        base_dir
            .map(|base_dir| base_dir.join(&candidate))
            .unwrap_or(candidate)
    } else {
        candidate
    }
}

pub fn load_baseline_library_document() -> Result<KirDocument, KirError> {
    Ok(BaselineLibraryConfig::bundled_stdlib().resolve()?.document)
}

pub fn write_kpar_package(path: &Path, package: &KparPackageBuild) -> Result<(), KirError> {
    if package.name.trim().is_empty() {
        return Err(KirError::Sysml(
            "package name must not be empty".to_string(),
        ));
    }
    if package.sources.is_empty() {
        return Err(KirError::Sysml(
            "package must contain at least one source file".to_string(),
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut seen_paths = BTreeSet::new();
    let mut sources = package.sources.clone();
    sources.sort_by(|left, right| left.path.cmp(&right.path));
    for source in &sources {
        validate_kpar_source_path(&source.path)?;
        if !seen_paths.insert(source.path.clone()) {
            return Err(KirError::Sysml(format!(
                "duplicate package source path: {}",
                source.path
            )));
        }
    }

    let file = std::fs::File::create(path)?;
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::FileOptions::default();

    writer
        .start_file(".project.json", options)
        .map_err(zip_error_to_kir_error)?;
    let mut project = serde_json::Map::new();
    project.insert(
        "name".to_string(),
        serde_json::Value::String(package.name.clone()),
    );
    if let Some(version) = &package.version {
        project.insert(
            "version".to_string(),
            serde_json::Value::String(version.clone()),
        );
    }
    project.insert("usage".to_string(), serde_json::Value::Array(Vec::new()));
    writer.write_all(serde_json::Value::Object(project).to_string().as_bytes())?;

    writer
        .start_file(".meta.json", options)
        .map_err(zip_error_to_kir_error)?;
    writer.write_all(br#"{"files":[]}"#)?;

    for source in &sources {
        writer
            .start_file(&source.path, options)
            .map_err(zip_error_to_kir_error)?;
        writer.write_all(source.content.as_bytes())?;
    }

    writer.finish().map_err(zip_error_to_kir_error)?;
    Ok(())
}

fn compile_sysml_directory(
    path: &Path,
    library_context: &KirDocument,
) -> Result<KirDocument, KirError> {
    let source_files = collect_sysml_directory_source_files(path)?;
    compile_library_source_files(source_files, library_context)
}

fn compile_kpar_file(
    path: &Path,
    library_context: &KirDocument,
) -> Result<(KirDocument, Option<KparProjectMetadata>), KirError> {
    let (source_files, package_metadata) = collect_kpar_source_files(path)?;
    let document = compile_library_source_files(source_files, library_context)?;
    Ok((document, package_metadata))
}

fn compile_kpar_package_set(
    path: &Path,
    entry: &str,
    library_context: &KirDocument,
) -> Result<(KirDocument, Option<KparProjectMetadata>), KirError> {
    let package_index = build_kpar_package_index(path)?;
    let entry_key = package_index.resolve(entry, None).ok_or_else(|| {
        KirError::Sysml(format!(
            "package-set entry '{entry}' not found in {}",
            path.display()
        ))
    })?;
    let mut visit_stack = Vec::new();
    let mut ordered_keys = Vec::new();
    let mut visited = BTreeSet::new();
    collect_package_order(
        &package_index,
        &entry_key,
        &mut visit_stack,
        &mut visited,
        &mut ordered_keys,
    )?;

    let mut merged_context = library_context.clone();
    let mut package_documents = Vec::new();
    let mut root_metadata = None;

    for package_key in ordered_keys {
        let package = package_index.packages.get(&package_key).ok_or_else(|| {
            KirError::Sysml(format!(
                "indexed package '{package_key}' missing from package set"
            ))
        })?;
        let (document, metadata) = compile_kpar_file(&package.path, &merged_context)?;
        if package_key == entry_key {
            root_metadata = metadata.clone();
        }
        merged_context = KirDocument::merge([merged_context, document.clone()])?;
        package_documents.push(document);
    }

    Ok((KirDocument::merge(package_documents)?, root_metadata))
}

fn compile_library_source_files(
    source_files: Vec<SourceDocument>,
    library_context: &KirDocument,
) -> Result<KirDocument, KirError> {
    compile_source_documents(source_files, library_context)
}

fn collect_sysml_directory_source_files(path: &Path) -> Result<Vec<SourceDocument>, KirError> {
    let mut files = Vec::new();
    collect_sysml_directory_source_files_recursive(path, path, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn collect_sysml_directory_source_files_recursive(
    root: &Path,
    current: &Path,
    files: &mut Vec<SourceDocument>,
) -> Result<(), KirError> {
    let mut entries = std::fs::read_dir(current)?.collect::<Result<Vec<_>, std::io::Error>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_sysml_directory_source_files_recursive(root, &path, files)?;
            continue;
        }

        if !is_library_source_file(&path) {
            continue;
        }

        let content = std::fs::read_to_string(&path)?;
        let source_name = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        files.push(SourceDocument::new(source_name, content));
    }

    Ok(())
}

fn digest_file(path: &Path) -> Result<String, KirError> {
    let mut file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(format_stable_digest([(
        "file".as_bytes(),
        bytes.as_slice(),
    )]))
}

fn digest_sysml_directory(path: &Path) -> Result<String, KirError> {
    let source_files = collect_sysml_directory_source_files(path)?;
    Ok(format_stable_digest(source_files.iter().flat_map(|file| {
        [
            ("path".as_bytes(), file.path.as_bytes()),
            ("content".as_bytes(), file.content.as_bytes()),
        ]
    })))
}

fn digest_kpar_directory(path: &Path) -> Result<String, KirError> {
    let mut entries = std::fs::read_dir(path)?.collect::<Result<Vec<_>, std::io::Error>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut items = Vec::new();

    for entry in entries {
        let package_path = entry.path();
        if package_path.extension().and_then(|value| value.to_str()) != Some("kpar") {
            continue;
        }

        let package_name = package_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();
        let mut file = std::fs::File::open(&package_path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        items.push((package_name, bytes));
    }

    Ok(format_stable_digest(items.iter().flat_map(
        |(name, bytes)| {
            [
                ("path".as_bytes(), name.as_bytes()),
                ("content".as_bytes(), bytes.as_slice()),
            ]
        },
    )))
}

fn format_stable_digest<'a, I>(chunks: I) -> String
where
    I: IntoIterator<Item = (&'a [u8], &'a [u8])>,
{
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for (label, bytes) in chunks {
        for byte in label
            .iter()
            .chain(&(bytes.len() as u64).to_le_bytes())
            .chain(bytes)
        {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }

    format!("fnv1a64:{hash:016x}")
}

fn collect_kpar_source_files(
    path: &Path,
) -> Result<(Vec<SourceDocument>, Option<KparProjectMetadata>), KirError> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(zip_error_to_kir_error)?;
    let mut files = Vec::new();
    let mut package_metadata = None;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(zip_error_to_kir_error)?;
        if !entry.is_file() {
            continue;
        }

        let entry_name = entry.name().replace('\\', "/");
        if entry_name == ".project.json" {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            package_metadata = serde_json::from_str(&content).ok();
            continue;
        }

        if !is_library_archive_source_entry(&entry_name) {
            continue;
        }

        let mut content = String::new();
        entry.read_to_string(&mut content)?;
        files.push(SourceDocument::new(entry_name, content));
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok((files, package_metadata))
}

fn build_kpar_package_index(path: &Path) -> Result<KparPackageIndex, KirError> {
    let mut packages = BTreeMap::new();
    let mut aliases = BTreeMap::<String, Vec<String>>::new();
    let mut entries = std::fs::read_dir(path)?.collect::<Result<Vec<_>, std::io::Error>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let package_path = entry.path();
        if package_path.extension().and_then(|value| value.to_str()) != Some("kpar") {
            continue;
        }

        let (_, metadata) = collect_kpar_source_files(&package_path)?;
        let package_key = package_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                KirError::Sysml(format!(
                    "failed to derive package file name from {}",
                    package_path.display()
                ))
            })?
            .to_string();
        let package = IndexedKparPackage {
            key: package_key.clone(),
            path: package_path,
            metadata,
        };

        for alias in package.aliases() {
            aliases.entry(alias).or_default().push(package_key.clone());
        }

        packages.insert(package_key, package);
    }

    Ok(KparPackageIndex { packages, aliases })
}

fn collect_package_order(
    index: &KparPackageIndex,
    package_key: &str,
    visit_stack: &mut Vec<String>,
    visited: &mut BTreeSet<String>,
    ordered_keys: &mut Vec<String>,
) -> Result<(), KirError> {
    if visited.contains(package_key) {
        return Ok(());
    }
    if visit_stack.iter().any(|entry| entry == package_key) {
        let cycle = visit_stack
            .iter()
            .cloned()
            .chain(std::iter::once(package_key.to_string()))
            .collect::<Vec<_>>()
            .join(" -> ");
        return Err(KirError::Sysml(format!(
            "cyclic package dependency detected: {cycle}"
        )));
    }

    let package = index.packages.get(package_key).ok_or_else(|| {
        KirError::Sysml(format!(
            "package '{package_key}' missing from package index"
        ))
    })?;
    visit_stack.push(package_key.to_string());

    if let Some(metadata) = &package.metadata {
        for dependency in &metadata.usage {
            let dependency_key = index
                .resolve(
                    &dependency.resource,
                    dependency.version_constraint.as_deref(),
                )
                .ok_or_else(|| {
                    KirError::Sysml(format!(
                        "failed to resolve package dependency '{}'{} in package '{}'",
                        dependency.resource,
                        dependency
                            .version_constraint
                            .as_deref()
                            .map(|version| format!(" @ {version}"))
                            .unwrap_or_default(),
                        package_key
                    ))
                })?;
            collect_package_order(index, &dependency_key, visit_stack, visited, ordered_keys)?;
        }
    }

    visit_stack.pop();
    visited.insert(package_key.to_string());
    ordered_keys.push(package_key.to_string());
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
struct KparProjectMetadata {
    name: Option<String>,
    version: Option<String>,
    #[serde(default)]
    usage: Vec<KparDependency>,
}

#[derive(Debug, Clone, Deserialize)]
struct KparDependency {
    resource: String,
    #[serde(default, rename = "versionConstraint")]
    version_constraint: Option<String>,
}

#[derive(Debug, Clone)]
struct IndexedKparPackage {
    key: String,
    path: PathBuf,
    metadata: Option<KparProjectMetadata>,
}

impl IndexedKparPackage {
    fn aliases(&self) -> Vec<String> {
        let mut aliases = BTreeSet::new();
        aliases.insert(normalize_package_alias(&self.key));

        if let Some(stem) = Path::new(&self.key)
            .file_stem()
            .and_then(|value| value.to_str())
        {
            aliases.insert(normalize_package_alias(stem));
            aliases.insert(normalize_package_alias(&strip_version_suffix(stem)));
        }

        if let Some(metadata) = &self.metadata {
            if let Some(name) = metadata.name.as_deref() {
                aliases.insert(normalize_package_alias(name));
                aliases.insert(normalize_package_alias(&strip_metadata_prefix(name)));
                aliases.insert(format!(
                    "{}.kpar",
                    normalize_package_alias(&strip_metadata_prefix(name))
                ));
            }
        }

        aliases
            .into_iter()
            .filter(|alias| !alias.is_empty())
            .collect()
    }
}

#[derive(Debug, Clone)]
struct KparPackageIndex {
    packages: BTreeMap<String, IndexedKparPackage>,
    aliases: BTreeMap<String, Vec<String>>,
}

impl KparPackageIndex {
    fn resolve(&self, reference: &str, version: Option<&str>) -> Option<String> {
        let reference_aliases = package_reference_aliases(reference);
        let mut matches = reference_aliases
            .into_iter()
            .filter_map(|alias| self.aliases.get(&alias))
            .flat_map(|entries| entries.iter().cloned())
            .collect::<BTreeSet<_>>();

        if let Some(version) = version {
            matches.retain(|package_key| {
                self.packages
                    .get(package_key)
                    .and_then(|package| package.metadata.as_ref())
                    .and_then(|metadata| metadata.version.as_deref())
                    == Some(version)
            });
        }

        if matches.len() == 1 {
            matches.into_iter().next()
        } else {
            None
        }
    }
}

fn default_baseline_library_id() -> String {
    "stdlib".to_string()
}

fn strip_version_suffix(value: &str) -> String {
    match value.rsplit_once('-') {
        Some((prefix, suffix))
            if suffix
                .chars()
                .all(|character| character.is_ascii_digit() || character == '.') =>
        {
            prefix.to_string()
        }
        _ => value.to_string(),
    }
}

fn strip_metadata_prefix(value: &str) -> String {
    value
        .trim()
        .strip_prefix("Kernel ")
        .or_else(|| value.trim().strip_prefix("SysML "))
        .unwrap_or(value.trim())
        .to_string()
}

fn normalize_package_alias(value: &str) -> String {
    value
        .trim()
        .replace('_', "-")
        .replace(' ', "-")
        .to_ascii_lowercase()
}

fn package_reference_aliases(reference: &str) -> Vec<String> {
    let mut aliases = BTreeSet::new();
    let trimmed = reference.trim();
    aliases.insert(normalize_package_alias(trimmed));

    if let Some(file_name) = trimmed.rsplit('/').next() {
        aliases.insert(normalize_package_alias(file_name));
        if let Some(stem) = Path::new(file_name)
            .file_stem()
            .and_then(|value| value.to_str())
        {
            aliases.insert(normalize_package_alias(stem));
            aliases.insert(normalize_package_alias(&strip_version_suffix(stem)));
        }
    }

    aliases
        .into_iter()
        .filter(|alias| !alias.is_empty())
        .collect()
}

fn validate_kpar_source_path(path: &str) -> Result<(), KirError> {
    let normalized = path.replace('\\', "/");
    if normalized.trim().is_empty() {
        return Err(KirError::Sysml(
            "package source path must not be empty".to_string(),
        ));
    }
    if normalized.starts_with('/') || normalized.contains("/../") || normalized.starts_with("../") {
        return Err(KirError::Sysml(format!(
            "package source path must be relative and stay inside the package: {path}"
        )));
    }
    if !is_library_archive_source_entry(&normalized) {
        return Err(KirError::Sysml(format!(
            "package source path must end in .sysml or .kerml: {path}"
        )));
    }
    Ok(())
}

fn zip_error_to_kir_error(error: zip::result::ZipError) -> KirError {
    KirError::Io(std::io::Error::other(error))
}

fn is_library_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("sysml" | "kerml")
    )
}

fn is_library_archive_source_entry(entry_name: &str) -> bool {
    entry_name.ends_with(".sysml") || entry_name.ends_with(".kerml")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io::Write;

    use serde_json::Value;

    use super::{
        BaselineLibraryConfig, KparPackageBuild, KparPackageSource, LibraryProviderConfig,
        write_kpar_package,
    };
    use crate::ir::{KirDocument, KirElement};

    #[test]
    fn bundled_baseline_config_defaults_to_stdlib_provider() {
        let config = BaselineLibraryConfig::default();

        assert_eq!(config.id, "stdlib");
        assert_eq!(config.provider, LibraryProviderConfig::BundledStdlib);
    }

    #[test]
    fn precompiled_kir_artifact_provider_resolves_document_from_file() {
        let temp_root =
            std::env::temp_dir().join(format!("mercurio-local-library-{}", std::process::id()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let kir_path = temp_root.join("sample.kir.json");
        let sample = KirDocument {
            metadata: BTreeMap::from([("source".to_string(), Value::String("test".to_string()))]),
            elements: vec![KirElement {
                id: "Demo::Thing".to_string(),
                kind: "PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::new(),
            }],
        };
        sample.write_pretty_to_path(&kir_path).unwrap();

        let artifact = LibraryProviderConfig::PrecompiledKirArtifact {
            path: kir_path.display().to_string(),
        }
        .resolve("demo")
        .unwrap();

        assert_eq!(artifact.library_id, "demo");
        assert_eq!(artifact.source_kind, "precompiled_kir_artifact");
        assert_eq!(artifact.document.elements.len(), 1);
        assert_eq!(artifact.document.elements[0].id, "Demo::Thing");

        std::fs::remove_file(kir_path).unwrap();
        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn precompiled_kir_artifact_provider_resolves_relative_to_base_dir() {
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-local-library-relative-{}",
            std::process::id()
        ));
        let base_dir = temp_root.join("project");
        std::fs::create_dir_all(&base_dir).unwrap();
        let kir_path = base_dir.join("baseline").join("sample.kir.json");
        let sample = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "Demo::RelativeThing".to_string(),
                kind: "PartDefinition".to_string(),
                layer: 1,
                properties: BTreeMap::new(),
            }],
        };
        sample.write_pretty_to_path(&kir_path).unwrap();

        let artifact = LibraryProviderConfig::PrecompiledKirArtifact {
            path: "baseline/sample.kir.json".to_string(),
        }
        .resolve_from("demo", Some(&base_dir))
        .unwrap();

        assert_eq!(artifact.document.elements[0].id, "Demo::RelativeThing");

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn deserializes_legacy_local_kir_file_alias() {
        let config: LibraryProviderConfig = serde_json::from_value(serde_json::json!({
            "kind": "local_kir_file",
            "path": "baseline/sample.kir.json"
        }))
        .unwrap();

        assert_eq!(
            config,
            LibraryProviderConfig::PrecompiledKirArtifact {
                path: "baseline/sample.kir.json".to_string()
            }
        );
    }

    #[test]
    fn sysml_directory_provider_compiles_source_backed_library() {
        let temp_root =
            std::env::temp_dir().join(format!("mercurio-sysml-library-{}", std::process::id()));
        let source_dir = temp_root.join("library");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(
            source_dir.join("domain.sysml"),
            "package Demo {\n  part def Thing;\n}\n",
        )
        .unwrap();

        let artifact = LibraryProviderConfig::SysmlDirectory {
            path: source_dir.display().to_string(),
        }
        .resolve("demo")
        .unwrap();

        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Demo.Thing")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn sysml_directory_provider_compiles_kerml_sources() {
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-kerml-directory-library-{}",
            std::process::id()
        ));
        let source_dir = temp_root.join("library");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(
            source_dir.join("kernel.kerml"),
            "package Kernel {\n  feature def SemanticThing;\n}\n",
        )
        .unwrap();

        let artifact = LibraryProviderConfig::SysmlDirectory {
            path: source_dir.display().to_string(),
        }
        .resolve("kernel-lib")
        .unwrap();

        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Kernel.SemanticThing")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn kpar_file_provider_compiles_source_backed_library() {
        let temp_root =
            std::env::temp_dir().join(format!("mercurio-kpar-library-{}", std::process::id()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let kpar_path = temp_root.join("domain-lib.kpar");
        write_test_kpar(
            &kpar_path,
            "Domain Library",
            "1.2.3",
            &[("domain.sysml", "package Domain {\n  part def Thing;\n}\n")],
        );

        let artifact = LibraryProviderConfig::KparFile {
            path: kpar_path.display().to_string(),
        }
        .resolve("domain-lib")
        .unwrap();

        assert_eq!(artifact.source_kind, "kpar_file");
        assert_eq!(
            artifact
                .cache_metadata
                .as_ref()
                .and_then(|metadata| metadata.source_version.as_deref()),
            Some("1.2.3")
        );
        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Domain.Thing")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn write_kpar_package_writes_source_backed_library() {
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-write-kpar-library-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_root).unwrap();
        let kpar_path = temp_root.join("domain-lib.kpar");

        write_kpar_package(
            &kpar_path,
            &KparPackageBuild {
                name: "Domain Library".to_string(),
                version: Some("1.2.3".to_string()),
                sources: vec![KparPackageSource {
                    path: "domain.sysml".to_string(),
                    content: "package Domain {\n  part def Thing;\n}\n".to_string(),
                }],
            },
        )
        .unwrap();

        let artifact = LibraryProviderConfig::KparFile {
            path: kpar_path.display().to_string(),
        }
        .resolve("domain-lib")
        .unwrap();

        assert_eq!(
            artifact
                .cache_metadata
                .as_ref()
                .and_then(|metadata| metadata.source_version.as_deref()),
            Some("1.2.3")
        );
        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Domain.Thing")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn kpar_file_provider_compiles_kerml_sources() {
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-kerml-kpar-library-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_root).unwrap();
        let kpar_path = temp_root.join("kernel-lib.kpar");
        write_test_kpar(
            &kpar_path,
            "Kernel Library",
            "1.2.3",
            &[(
                "kernel.kerml",
                "package Kernel {\n  feature def SemanticThing;\n}\n",
            )],
        );

        let artifact = LibraryProviderConfig::KparFile {
            path: kpar_path.display().to_string(),
        }
        .resolve("kernel-lib")
        .unwrap();

        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Kernel.SemanticThing")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn package_set_directory_provider_resolves_dependency_closure() {
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-package-set-library-{}",
            std::process::id()
        ));
        let package_dir = temp_root.join("package-set");
        std::fs::create_dir_all(&package_dir).unwrap();

        write_test_kpar_with_usage(
            &package_dir.join("Kernel_Semantic_Library-1.0.0.kpar"),
            "Kernel Semantic Library",
            "1.0.0",
            &[],
            &[(
                "semantic.kerml",
                "package Kernel {\n  feature def SemanticThing;\n}\n",
            )],
        );
        write_test_kpar_with_usage(
            &package_dir.join("SysML_Systems_Library-2.0.0.kpar"),
            "SysML Systems Library",
            "2.0.0",
            &[(
                "https://www.omg.org/spec/KerML/20250201/Semantic-Library.kpar",
                "1.0.0",
            )],
            &[(
                "systems.sysml",
                "package Systems {\n  part def SystemThing;\n}\n",
            )],
        );

        let artifact = LibraryProviderConfig::PackageSetDirectory {
            path: package_dir.display().to_string(),
            entry: "https://www.omg.org/spec/SysML/20250201/Systems-Library.kpar".to_string(),
        }
        .resolve("systems")
        .unwrap();

        assert_eq!(artifact.source_kind, "package_set_directory");
        assert_eq!(
            artifact
                .cache_metadata
                .as_ref()
                .and_then(|metadata| metadata.source_version.as_deref()),
            Some("2.0.0")
        );
        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Kernel.SemanticThing")
        );
        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Systems.SystemThing")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    fn write_test_kpar(
        path: &std::path::Path,
        name: &str,
        version: &str,
        entries: &[(&str, &str)],
    ) {
        write_test_kpar_with_usage(path, name, version, &[], entries);
    }

    fn write_test_kpar_with_usage(
        path: &std::path::Path,
        name: &str,
        version: &str,
        usage: &[(&str, &str)],
        entries: &[(&str, &str)],
    ) {
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default();

        writer.start_file(".project.json", options).unwrap();
        writer
            .write_all(
                serde_json::json!({
                    "name": name,
                    "version": version,
                    "usage": usage
                        .iter()
                        .map(|(resource, version_constraint)| serde_json::json!({
                            "resource": resource,
                            "versionConstraint": version_constraint
                        }))
                        .collect::<Vec<_>>()
                })
                .to_string()
                .as_bytes(),
            )
            .unwrap();

        writer.start_file(".meta.json", options).unwrap();
        writer.write_all(br#"{"files":[]}"#).unwrap();

        for (entry_name, content) in entries {
            writer.start_file(*entry_name, options).unwrap();
            writer.write_all(content.as_bytes()).unwrap();
        }

        writer.finish().unwrap();
    }
}
