use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::authoring::{
    AuthoringProject, ContainerSelector, Mutation, QualifiedName, create_empty_model,
    load_authoring_project_from_sysml,
};
use crate::feasibility::{
    CoreMutationFeasibilityService, FeasibilityStatus, MutationContext, MutationFeasibilityReport,
    MutationFeasibilityService,
};
use crate::frontend::sysml::compile_sysml_text;
use crate::ir::KirDocument;
use crate::mutation::{
    ElementRef, MutationEvidence, MutationProposal, SemanticDiff, SemanticMutation,
    diff_kir_documents,
};
use crate::paths::default_stdlib_path;
use crate::session::{CommitMode, ModelFork, ModelWorkspace, WorkspaceSnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreScalabilityCreationStrategy {
    SessionOverlay,
    Mutators,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreScalabilityMetricConfig {
    pub model_sizes: Vec<usize>,
    pub edit_count: usize,
    pub target_file: String,
    pub package_name: String,
    pub creation_strategy: CoreScalabilityCreationStrategy,
}

impl Default for CoreScalabilityMetricConfig {
    fn default() -> Self {
        Self {
            model_sizes: vec![100, 1_000, 10_000],
            edit_count: 100,
            target_file: "scalability.sysml".to_string(),
            package_name: "Scalability".to_string(),
            creation_strategy: CoreScalabilityCreationStrategy::SessionOverlay,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CoreScalabilityReport {
    pub generated_at_unix_seconds: u64,
    pub target_file: String,
    pub package_name: String,
    pub creation_strategy: CoreScalabilityCreationStrategy,
    pub edit_count_requested: usize,
    pub scenarios: Vec<CoreScalabilityScenarioReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoreScalabilityScenarioReport {
    pub model_size: usize,
    pub edit_count: usize,
    pub source_file: String,
    pub sysml_bytes: usize,
    pub edited_sysml_bytes: usize,
    pub kir_elements_before: usize,
    pub kir_elements_after: usize,
    pub diff_summary: SemanticDiffSummary,
    pub timings: CoreScalabilityTimings,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoreScalabilityTimings {
    pub create_model: TimingMetric,
    pub save_sysml: TimingMetric,
    pub reload_sysml: TimingMetric,
    pub compile_before_kir: TimingMetric,
    pub prepare_edits: TimingMetric,
    pub apply_edits: TimingMetric,
    pub reload_edited_sysml: TimingMetric,
    pub compile_after_kir: TimingMetric,
    pub diff_kir: TimingMetric,
    pub total: TimingMetric,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimingMetric {
    pub millis: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticDiffSummary {
    pub added_elements: usize,
    pub removed_elements: usize,
    pub renamed_elements: usize,
    pub moved_elements: usize,
    pub retyped_usages: usize,
    pub changed_specializations: usize,
    pub changed_attributes: usize,
    pub added_relationships: usize,
    pub removed_relationships: usize,
}

pub fn run_core_scalability_metric(
    config: CoreScalabilityMetricConfig,
) -> Result<CoreScalabilityReport, Box<dyn Error>> {
    let generated_at_unix_seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let stdlib = KirDocument::from_path(Path::new(&default_stdlib_path()))?;
    let mut scenarios = Vec::new();

    for model_size in config.model_sizes.iter().copied() {
        scenarios.push(run_scenario(&config, model_size, &stdlib)?);
    }

    Ok(CoreScalabilityReport {
        generated_at_unix_seconds,
        target_file: config.target_file,
        package_name: config.package_name,
        creation_strategy: config.creation_strategy,
        edit_count_requested: config.edit_count,
        scenarios,
    })
}

fn run_scenario(
    config: &CoreScalabilityMetricConfig,
    model_size: usize,
    stdlib: &KirDocument,
) -> Result<CoreScalabilityScenarioReport, Box<dyn Error>> {
    let total_timer = Instant::now();
    match config.creation_strategy {
        CoreScalabilityCreationStrategy::SessionOverlay => {
            run_session_overlay_scenario(config, model_size, stdlib, total_timer)
        }
        CoreScalabilityCreationStrategy::Mutators => {
            run_mutator_scenario(config, model_size, stdlib, total_timer)
        }
    }
}

fn run_session_overlay_scenario(
    config: &CoreScalabilityMetricConfig,
    model_size: usize,
    stdlib: &KirDocument,
    total_timer: Instant,
) -> Result<CoreScalabilityScenarioReport, Box<dyn Error>> {
    let ((workspace, fork), create_model) =
        measure(|| create_model_with_session_overlay(model_size, config.package_name.as_str()))?;

    let (commit, save_sysml) = measure(|| fork.commit(CommitMode::RewriteSource))?;
    let (source_file, sysml_text) = commit
        .edited_files
        .iter()
        .next()
        .map(|(path, source)| (path.clone(), source.clone()))
        .ok_or("session commit did not emit source")?;
    let sysml_bytes = sysml_text.len();

    let (_, reload_sysml) = measure(|| {
        load_authoring_project_from_sysml(BTreeMap::from([(
            source_file.clone(),
            sysml_text.clone(),
        )]))
    })?;

    let (before_kir, compile_before_kir) =
        measure(|| compile_sysml_text(&sysml_text, source_file.as_str(), stdlib))?;

    let edit_count = config.edit_count.min(model_size);
    let ((after_kir, edited_diff), (prepare_edits, apply_edits)) =
        run_session_overlay_edits(&workspace, config.package_name.as_str(), edit_count)?;
    let reload_edited_sysml = TimingMetric::zero();
    let compile_after_kir = TimingMetric::zero();
    let (_, diff_kir) = measure(|| Ok::<SemanticDiff, Box<dyn Error>>(edited_diff.clone()))?;
    let total = TimingMetric::from_elapsed(total_timer.elapsed());

    Ok(CoreScalabilityScenarioReport {
        model_size,
        edit_count,
        source_file,
        sysml_bytes,
        edited_sysml_bytes: 0,
        kir_elements_before: before_kir.elements.len(),
        kir_elements_after: after_kir.elements.len(),
        diff_summary: SemanticDiffSummary::from_diff(&edited_diff),
        timings: CoreScalabilityTimings {
            create_model,
            save_sysml,
            reload_sysml,
            compile_before_kir,
            prepare_edits,
            apply_edits,
            reload_edited_sysml,
            compile_after_kir,
            diff_kir,
            total,
        },
    })
}

fn run_mutator_scenario(
    config: &CoreScalabilityMetricConfig,
    model_size: usize,
    stdlib: &KirDocument,
    total_timer: Instant,
) -> Result<CoreScalabilityScenarioReport, Box<dyn Error>> {
    let ((mut project, changed_files), create_model) = measure(|| {
        create_model_with_mutators(
            model_size,
            config.target_file.as_str(),
            config.package_name.as_str(),
        )
    })?;

    let (sysml_text, save_sysml) =
        measure(|| save_sysml(&mut project, &changed_files, config.target_file.as_str()))?;
    let sysml_bytes = sysml_text.len();

    let (reloaded, reload_sysml) = measure(|| {
        load_authoring_project_from_sysml(BTreeMap::from([(
            config.target_file.clone(),
            sysml_text.clone(),
        )]))
    })?;

    let (before_kir, compile_before_kir) =
        measure(|| compile_sysml_text(&sysml_text, config.target_file.as_str(), stdlib))?;

    let edit_count = config.edit_count.min(model_size);
    let context = MutationContext::from_project(reloaded);
    let proposal = rename_proposal(&context, config.package_name.as_str(), edit_count);
    let service = CoreMutationFeasibilityService::new();

    let (feasibility, prepare_edits) = measure(|| {
        Ok::<MutationFeasibilityReport, Box<dyn Error>>(service.check(&context, &proposal))
    })?;
    if !matches!(
        feasibility.status,
        FeasibilityStatus::Allowed | FeasibilityStatus::AllowedWithWarnings
    ) {
        return Err(format!("edit feasibility failed: {feasibility:#?}").into());
    }
    let plan = feasibility
        .normalized_plan
        .as_ref()
        .ok_or("feasibility allowed edits without a normalized plan")?;

    let (application, apply_edits) = measure(|| {
        service
            .apply_checked_plan(&context, plan)
            .map_err(|issue| format!("{issue:#?}"))
    })?;
    let edited_sysml_text = application
        .edited_files
        .get(&config.target_file)
        .cloned()
        .ok_or_else(|| format!("missing edited file `{}`", config.target_file))?;
    let edited_sysml_bytes = edited_sysml_text.len();

    let (_, reload_edited_sysml) = measure(|| {
        load_authoring_project_from_sysml(BTreeMap::from([(
            config.target_file.clone(),
            edited_sysml_text.clone(),
        )]))
    })?;

    let (after_kir, compile_after_kir) =
        measure(|| compile_sysml_text(&edited_sysml_text, config.target_file.as_str(), stdlib))?;

    let (diff, diff_kir) = measure(|| {
        Ok::<SemanticDiff, Box<dyn Error>>(diff_kir_documents(&before_kir, &after_kir))
    })?;
    let total = TimingMetric::from_elapsed(total_timer.elapsed());

    Ok(CoreScalabilityScenarioReport {
        model_size,
        edit_count,
        source_file: config.target_file.clone(),
        sysml_bytes,
        edited_sysml_bytes,
        kir_elements_before: before_kir.elements.len(),
        kir_elements_after: after_kir.elements.len(),
        diff_summary: SemanticDiffSummary::from_diff(&diff),
        timings: CoreScalabilityTimings {
            create_model,
            save_sysml,
            reload_sysml,
            compile_before_kir,
            prepare_edits,
            apply_edits,
            reload_edited_sysml,
            compile_after_kir,
            diff_kir,
            total,
        },
    })
}

fn create_model_with_session_overlay(
    model_size: usize,
    package_name: &str,
) -> Result<(ModelWorkspace, ModelFork), Box<dyn Error>> {
    let workspace = ModelWorkspace::new(WorkspaceSnapshot::new(KirDocument {
        metadata: BTreeMap::new(),
        elements: Vec::new(),
    })?);
    let session = workspace.session();
    let mut fork = session.fork("core scalability session overlay creation");
    let package = fork.package(package_name, None)?;
    for index in 0..model_size {
        fork.requirement(
            &package,
            element_name(index),
            format!("Generated scalability record {index:05}"),
        )?;
    }
    Ok((workspace, fork))
}

fn run_session_overlay_edits(
    workspace: &ModelWorkspace,
    package_name: &str,
    edit_count: usize,
) -> Result<((KirDocument, SemanticDiff), (TimingMetric, TimingMetric)), Box<dyn Error>> {
    let session = workspace.session();
    let mut fork = session.fork("core scalability session overlay edits");
    let (_, prepare_edits) = measure(|| {
        for index in 0..edit_count {
            fork.rename_declaration(
                format!("{package_name}.{}", element_name(index)),
                edited_element_name(index),
            )?;
        }
        Ok::<(), Box<dyn Error>>(())
    })?;
    let ((after_kir, diff), apply_edits) = measure(|| {
        let after_kir = fork.materialize()?;
        let diff = diff_kir_documents(&workspace.current_snapshot().kir, &after_kir);
        Ok::<_, Box<dyn Error>>((after_kir, diff))
    })?;
    Ok(((after_kir, diff), (prepare_edits, apply_edits)))
}

fn create_model_with_mutators(
    model_size: usize,
    target_file: &str,
    package_name: &str,
) -> Result<(AuthoringProject, BTreeSet<String>), Box<dyn Error>> {
    let mut project = create_empty_model();
    let mut changed_files = BTreeSet::new();

    let result = project.apply_mutation(Mutation::AddPackage {
        target_file: target_file.to_string(),
        package_name: QualifiedName::parse(package_name),
    })?;
    changed_files.extend(result.changed_files);

    for index in 0..model_size {
        let result = project.apply_mutation(Mutation::AddDefinition {
            container: ContainerSelector::Package {
                qualified_name: QualifiedName::parse(package_name),
            },
            keyword: "part".to_string(),
            name: element_name(index),
            specializes: Vec::new(),
        })?;
        changed_files.extend(result.changed_files);
    }

    Ok((project, changed_files))
}

fn save_sysml(
    project: &mut AuthoringProject,
    changed_files: &BTreeSet<String>,
    target_file: &str,
) -> Result<String, Box<dyn Error>> {
    let write_back = project.write_back_changed_files_and_update(changed_files)?;
    write_back
        .edited_files
        .get(target_file)
        .cloned()
        .ok_or_else(|| format!("missing rendered file `{target_file}`").into())
}

fn rename_proposal(
    context: &MutationContext,
    package_name: &str,
    edit_count: usize,
) -> MutationProposal {
    let operations = (0..edit_count)
        .map(|index| SemanticMutation::RenameDeclaration {
            element: ElementRef::new(format!("{package_name}.{}", element_name(index))),
            new_name: edited_element_name(index),
        })
        .collect::<Vec<_>>();

    MutationProposal {
        intent: format!("Rename {edit_count} generated scalability records"),
        affected_elements: operations
            .iter()
            .filter_map(|operation| match operation {
                SemanticMutation::RenameDeclaration { element, .. } => Some(element.clone()),
                _ => None,
            })
            .collect(),
        operations,
        evidence: vec![MutationEvidence {
            element: Some(ElementRef::new(package_name.to_string())),
            summary: "Synthetic core scalability metric workload.".to_string(),
        }],
        rationale: Some(
            "Measure feasibility and edit scalability on generated records.".to_string(),
        ),
        workspace_revision: context.workspace_revision.clone(),
    }
}

fn element_name(index: usize) -> String {
    format!("Component{index:05}")
}

fn edited_element_name(index: usize) -> String {
    format!("EditedComponent{index:05}")
}

fn measure<T, E, F>(operation: F) -> Result<(T, TimingMetric), E>
where
    F: FnOnce() -> Result<T, E>,
{
    let start = Instant::now();
    let output = operation()?;
    Ok((output, TimingMetric::from_elapsed(start.elapsed())))
}

impl TimingMetric {
    fn zero() -> Self {
        Self { millis: 0.0 }
    }

    fn from_elapsed(elapsed: std::time::Duration) -> Self {
        Self {
            millis: elapsed.as_secs_f64() * 1_000.0,
        }
    }
}

impl SemanticDiffSummary {
    fn from_diff(diff: &SemanticDiff) -> Self {
        Self {
            added_elements: diff.added_elements.len(),
            removed_elements: diff.removed_elements.len(),
            renamed_elements: diff.renamed_elements.len(),
            moved_elements: diff.moved_elements.len(),
            retyped_usages: diff.retyped_usages.len(),
            changed_specializations: diff.changed_specializations.len(),
            changed_attributes: diff.changed_attributes.len(),
            added_relationships: diff.added_relationships.len(),
            removed_relationships: diff.removed_relationships.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CoreScalabilityCreationStrategy, CoreScalabilityMetricConfig, run_core_scalability_metric,
    };

    #[test]
    fn scalability_metric_runs_tiny_session_overlay_model() {
        let report = run_core_scalability_metric(CoreScalabilityMetricConfig {
            model_sizes: vec![3],
            edit_count: 2,
            target_file: "tiny.sysml".to_string(),
            package_name: "Tiny".to_string(),
            creation_strategy: CoreScalabilityCreationStrategy::SessionOverlay,
        })
        .expect("metric runs");

        assert_eq!(
            report.creation_strategy,
            CoreScalabilityCreationStrategy::SessionOverlay
        );
        assert_eq!(report.scenarios.len(), 1);
        assert_eq!(report.scenarios[0].model_size, 3);
        assert_eq!(report.scenarios[0].edit_count, 2);
        assert_eq!(report.scenarios[0].diff_summary.changed_attributes, 2);
    }

    #[test]
    fn scalability_metric_still_supports_mutator_comparison() {
        let report = run_core_scalability_metric(CoreScalabilityMetricConfig {
            model_sizes: vec![3],
            edit_count: 2,
            target_file: "tiny.sysml".to_string(),
            package_name: "Tiny".to_string(),
            creation_strategy: CoreScalabilityCreationStrategy::Mutators,
        })
        .expect("metric runs");

        assert_eq!(
            report.creation_strategy,
            CoreScalabilityCreationStrategy::Mutators
        );
        assert_eq!(report.scenarios[0].diff_summary.removed_elements, 2);
        assert_eq!(report.scenarios[0].diff_summary.added_elements, 2);
    }
}
