use mercurio_core::{
    AttributeWritePolicy, AuthoringProject, ContainerSelector, Mutation, QualifiedName,
    SemanticEdit, WriteBackMode, create_empty_model, load_authoring_project_from_sysml,
};
use serde_json::json;

fn qname(value: &str) -> QualifiedName {
    QualifiedName::parse(value)
}

#[test]
fn empty_project_can_emit_new_sysml_file_after_mutation() {
    let mut project = create_empty_model();
    let package_result = project
        .apply_mutation(Mutation::AddPackage {
            target_file: "model.sysml".to_string(),
            package_name: qname("Demo"),
        })
        .unwrap();
    project.write_back_mutation(&package_result).unwrap();

    let definition_result = project
        .apply_mutation(Mutation::AddDefinition {
            container: ContainerSelector::Package {
                qualified_name: qname("Demo"),
            },
            keyword: "part".to_string(),
            name: "Vehicle".to_string(),
            specializes: Vec::new(),
        })
        .unwrap();
    let write_back = project.write_back_mutation(&definition_result).unwrap();
    let text = write_back.edited_files.get("model.sysml").unwrap();

    assert_eq!(write_back.mode, WriteBackMode::LocalizedPatch);
    assert!(text.contains("package Demo {"));
    assert!(text.contains("part def Vehicle;"));
    assert!(write_back.validation.ok);
}

#[test]
fn rename_rewrites_existing_definition_locally() {
    let source = "package Demo {\n  // untouched\n  part def Vehicle;\n}\n";
    let mut project = load_authoring_project_from_sysml(
        [("model.sysml".to_string(), source.to_string())]
            .into_iter()
            .collect(),
    )
    .unwrap();

    let mutation = project
        .apply_mutation(Mutation::RenameDeclaration {
            qualified_name: qname("Demo.Vehicle"),
            new_name: "Car".to_string(),
        })
        .unwrap();
    let write_back = project.write_back_mutation(&mutation).unwrap();
    let text = write_back.edited_files.get("model.sysml").unwrap();

    assert_eq!(write_back.mode, WriteBackMode::LocalizedPatch);
    assert!(text.contains("// untouched"));
    assert!(text.contains("part def Car;"));
    assert!(!text.contains("part def Vehicle;"));
    assert!(write_back.validation.ok);
}

#[test]
fn adding_nested_usage_rewrites_owner_container() {
    let source =
        "package Demo {\n  // file comment\n  part def Engine;\n  part def Vehicle {\n  }\n}\n";
    let mut project = load_authoring_project_from_sysml(
        [("model.sysml".to_string(), source.to_string())]
            .into_iter()
            .collect(),
    )
    .unwrap();

    let mutation = project
        .apply_mutation(Mutation::AddUsage {
            container: ContainerSelector::Declaration {
                qualified_name: qname("Demo.Vehicle"),
            },
            keyword: "part".to_string(),
            name: "engine".to_string(),
            ty: Some(qname("Engine")),
            specializes: Vec::new(),
        })
        .unwrap();
    let write_back = project.write_back_mutation(&mutation).unwrap();
    let text = write_back.edited_files.get("model.sysml").unwrap();

    assert_eq!(write_back.mode, WriteBackMode::LocalizedPatch);
    assert!(text.contains("// file comment"));
    assert!(text.contains("part engine: Engine;"));
    assert!(write_back.validation.ok);
}

#[test]
fn multi_file_addition_edits_only_target_file() {
    let files = [
        (
            "a.sysml".to_string(),
            "package A {\n  part def Vehicle;\n}\n".to_string(),
        ),
        (
            "b.sysml".to_string(),
            "package B {\n  part def Engine;\n}\n".to_string(),
        ),
    ]
    .into_iter()
    .collect();
    let mut project = AuthoringProject::from_sysml_files(files).unwrap();

    let mutation = project
        .apply_mutation(Mutation::AddDefinition {
            container: ContainerSelector::Package {
                qualified_name: qname("B"),
            },
            keyword: "part".to_string(),
            name: "Brake".to_string(),
            specializes: Vec::new(),
        })
        .unwrap();
    let write_back = project.write_back_mutation(&mutation).unwrap();

    assert!(write_back.edited_files.contains_key("b.sysml"));
    assert!(!write_back.edited_files.contains_key("a.sysml"));
    assert!(write_back.edited_files["b.sysml"].contains("part def Brake;"));
    assert!(write_back.validation.ok);
}

#[test]
fn moving_declaration_between_files_updates_source_and_destination() {
    let files = [
        (
            "a.sysml".to_string(),
            "package A {\n  part def Vehicle;\n}\n".to_string(),
        ),
        ("b.sysml".to_string(), "package B {\n}\n".to_string()),
    ]
    .into_iter()
    .collect();
    let mut project = AuthoringProject::from_sysml_files(files).unwrap();

    let mutation = project
        .apply_mutation(Mutation::MoveDeclaration {
            qualified_name: qname("A.Vehicle"),
            destination: ContainerSelector::Package {
                qualified_name: qname("B"),
            },
        })
        .unwrap();
    let write_back = project.write_back_mutation(&mutation).unwrap();

    assert!(write_back.edited_files["a.sysml"].contains("package A {\n}\n"));
    assert!(write_back.edited_files["b.sysml"].contains("part def Vehicle;"));
    assert!(write_back.validation.ok);
}

#[test]
fn semantic_declared_name_edit_maps_to_rename() {
    let source = "package Demo {\n  part def Vehicle;\n}\n";
    let mut project = load_authoring_project_from_sysml(
        [("model.sysml".to_string(), source.to_string())]
            .into_iter()
            .collect(),
    )
    .unwrap();

    let mutation = project
        .apply_semantic_edit(SemanticEdit::SetAttribute {
            element: qname("Demo.Vehicle"),
            attribute: "declaredName".to_string(),
            value: json!("Car"),
            policy: AttributeWritePolicy::DirectOnly,
        })
        .unwrap();
    let write_back = project.write_back_mutation(&mutation).unwrap();

    assert!(write_back.edited_files["model.sysml"].contains("part def Car;"));
    assert!(write_back.validation.ok);
}

#[test]
fn semantic_type_edit_updates_usage_type() {
    let source =
        "package Demo {\n  part def Engine;\n  part def Vehicle {\n    part engine;\n  }\n}\n";
    let mut project = load_authoring_project_from_sysml(
        [("model.sysml".to_string(), source.to_string())]
            .into_iter()
            .collect(),
    )
    .unwrap();

    let mutation = project
        .apply_semantic_edit(SemanticEdit::SetAttribute {
            element: qname("Demo.Vehicle.engine"),
            attribute: "type".to_string(),
            value: json!("Engine"),
            policy: AttributeWritePolicy::UpsertDirect,
        })
        .unwrap();
    let write_back = project.write_back_mutation(&mutation).unwrap();

    assert!(write_back.edited_files["model.sysml"].contains("part engine: Engine;"));
    assert!(write_back.validation.ok);
}

#[test]
fn semantic_specialization_add_edit_updates_definition() {
    let source = "package Demo {\n  part def Base;\n  part def Vehicle;\n}\n";
    let mut project = load_authoring_project_from_sysml(
        [("model.sysml".to_string(), source.to_string())]
            .into_iter()
            .collect(),
    )
    .unwrap();

    let mutation = project
        .apply_semantic_edit(SemanticEdit::AddAttributeValue {
            element: qname("Demo.Vehicle"),
            attribute: "specializes".to_string(),
            value: json!("Base"),
            policy: AttributeWritePolicy::UpsertDirect,
        })
        .unwrap();
    let write_back = project.write_back_mutation(&mutation).unwrap();

    assert!(write_back.edited_files["model.sysml"].contains("part def Vehicle specializes Base;"));
    assert!(write_back.validation.ok);
}

#[test]
fn semantic_modifier_edit_sets_abstract() {
    let source = "package Demo {\n  part def Vehicle;\n}\n";
    let mut project = load_authoring_project_from_sysml(
        [("model.sysml".to_string(), source.to_string())]
            .into_iter()
            .collect(),
    )
    .unwrap();

    let mutation = project
        .apply_semantic_edit(SemanticEdit::SetAttribute {
            element: qname("Demo.Vehicle"),
            attribute: "isAbstract".to_string(),
            value: json!(true),
            policy: AttributeWritePolicy::UpsertDirect,
        })
        .unwrap();
    let write_back = project.write_back_mutation(&mutation).unwrap();

    assert!(write_back.edited_files["model.sysml"].contains("abstract part def Vehicle;"));
    assert!(write_back.validation.ok);
}

#[test]
fn semantic_direct_only_policy_rejects_creating_missing_direct_value() {
    let source =
        "package Demo {\n  part def Engine;\n  part def Vehicle {\n    part engine;\n  }\n}\n";
    let mut project = load_authoring_project_from_sysml(
        [("model.sysml".to_string(), source.to_string())]
            .into_iter()
            .collect(),
    )
    .unwrap();

    let error = project
        .apply_semantic_edit(SemanticEdit::SetAttribute {
            element: qname("Demo.Vehicle.engine"),
            attribute: "type".to_string(),
            value: json!("Engine"),
            policy: AttributeWritePolicy::DirectOnly,
        })
        .unwrap_err();

    assert!(error.to_string().contains("UpsertDirect"));
}
