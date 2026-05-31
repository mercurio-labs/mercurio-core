use std::fs;
use std::path::{Path, PathBuf};

use mercurio_core::{
    KirDocument, PROJECT_DESCRIPTOR_FILE_NAME, SourceLanguage, default_kernel_library_path,
    default_sysml_library_path, load_model_stack, load_model_stack_with_language,
    resolve_project_context_for_language,
};
use serde_json::json;

#[test]
fn descriptorless_kerml_project_uses_kernel_baseline() {
    let root = temp_dir("descriptorless_kerml_project_uses_kernel_baseline");
    let model_path = root.join("models").join("demo.kerml");
    fs::create_dir_all(model_path.parent().unwrap()).unwrap();
    fs::write(&model_path, "package Demo { classifier Vehicle; }\n").unwrap();

    let context = resolve_project_context_for_language(&model_path, Some(SourceLanguage::Kerml))
        .expect("KerML project context should resolve");

    assert!(context.descriptor_path.is_none());
    assert_eq!(context.resolved_libraries.len(), 1);
    assert_eq!(context.resolved_libraries[0].id, "kernel");
    assert_eq!(
        context.resolved_libraries[0].source_path.as_deref(),
        Some(default_kernel_library_path().as_path())
    );
    assert_eq!(
        merged_library_id(&context.library_context_document),
        Some("org.omg/kerml-kernel")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn descriptorless_sysml_project_uses_sysml_baseline() {
    let root = temp_dir("descriptorless_sysml_project_uses_sysml_baseline");
    let model_path = root.join("models").join("demo.sysml");
    fs::create_dir_all(model_path.parent().unwrap()).unwrap();
    fs::write(&model_path, "package Demo { part def Vehicle; }\n").unwrap();

    let context = resolve_project_context_for_language(&model_path, Some(SourceLanguage::Sysml))
        .expect("SysML project context should resolve");

    assert!(context.descriptor_path.is_none());
    assert_eq!(context.resolved_libraries.len(), 1);
    assert_eq!(context.resolved_libraries[0].id, "stdlib");
    assert!(context.library_context_document.elements.len() > 100);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_descriptor_baseline_overrides_language_default() {
    let root = temp_dir("project_descriptor_baseline_overrides_language_default");
    let baseline_path = root.join("baseline.kir.json");
    write_kir(
        &baseline_path,
        "local/baseline",
        vec![json!({
            "id": "type.LocalBaseline",
            "kind": "KerML::Core::Type",
            "layer": 0,
            "properties": {}
        })],
    );
    fs::write(
        root.join(PROJECT_DESCRIPTOR_FILE_NAME),
        format!(
            r#"{{
  "libraries": [
    {{
      "id": "local",
      "role": "baseline",
      "provider": {{ "kind": "local_kir_file", "path": "{}" }}
    }}
  ]
}}"#,
            baseline_path.file_name().unwrap().to_string_lossy()
        ),
    )
    .unwrap();

    let model_path = root.join("demo.kerml");
    fs::write(&model_path, "package Demo { classifier Vehicle; }\n").unwrap();

    let context = resolve_project_context_for_language(&model_path, Some(SourceLanguage::Kerml))
        .expect("descriptor baseline should resolve");

    assert!(context.descriptor_path.is_some());
    assert_eq!(context.resolved_libraries.len(), 1);
    assert_eq!(context.resolved_libraries[0].id, "local");
    assert_eq!(
        merged_library_id(&context.library_context_document),
        Some("local/baseline")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn kir_json_loads_without_language_baseline_merge() {
    let root = temp_dir("kir_json_loads_without_language_baseline_merge");
    let kir_path = root.join("model.kir.json");
    write_kir(
        &kir_path,
        "test/raw",
        vec![json!({
            "id": "type.RawOnly",
            "kind": "KerML::Core::Type",
            "layer": 2,
            "properties": {}
        })],
    );

    let document =
        load_model_stack_with_language(&kir_path, SourceLanguage::Sysml).expect("raw KIR loads");

    assert_eq!(document.elements.len(), 1);
    assert!(
        document
            .elements
            .iter()
            .any(|element| element.id == "type.RawOnly")
    );
    assert!(
        !document
            .elements
            .iter()
            .any(|element| element.id == "SysML::Systems::PartDefinition")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_json_model_stack_still_merges_sysml_baseline() {
    let document = load_model_stack(&repo_path("test_files/examples/vehicle_model.json"))
        .expect("legacy JSON model stack loads");

    assert!(
        document
            .elements
            .iter()
            .any(|element| element.id == "SysML::Systems::PartDefinition")
    );
}

#[test]
fn bundled_sysml_library_loads_as_raw_kir() {
    let document = load_model_stack(&default_sysml_library_path()).expect("SysML library loads");

    assert!(document.elements.len() > 100);
    assert_eq!(
        document
            .metadata
            .get("merged_sources")
            .and_then(|value| value.as_array()),
        None
    );
}

fn write_kir(path: &Path, library_id: &str, elements: Vec<serde_json::Value>) {
    let document = json!({
        "metadata": {
            "library_id": library_id
        },
        "elements": elements
    });
    fs::write(path, serde_json::to_string_pretty(&document).unwrap()).unwrap();
}

fn merged_library_id(document: &KirDocument) -> Option<&str> {
    document
        .metadata
        .get("merged_sources")
        .and_then(|value| value.as_array())
        .and_then(|sources| sources.first())
        .and_then(|source| source.get("library_id"))
        .and_then(|value| value.as_str())
}

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(relative)
}

fn temp_dir(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("mercurio-{name}-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    fs::create_dir_all(&root).unwrap();
    root
}
