use std::collections::BTreeMap;

use mercurio_core::behavior::{StateMachineModel, StateNode};
use mercurio_core::graph::Element;
use mercurio_core::project_state_machines;
use mercurio_core::runtime::Runtime;
use mercurio_core::views::{RequirementSourceDto, RequirementTableRowDto, requirements_table_view};
use mercurio_reasoner_api::{
    CapabilityDescriptor, CapabilityKind, EvidenceGraph, EvidenceNode, EvidenceNodeKind,
    FindingSeverity, REASONING_API_VERSION, ReasoningArtifact, ReasoningFinding, ReasoningReport,
    ReasoningStatus, SemanticContextRef, SemanticElementRef, SourceSpanRef,
};
use serde_json::{Value, json};

pub const REQUIREMENT_COVERAGE_CAPABILITY_ID: &str = "mercurio.requirement.coverage";
pub const SEMANTIC_IMPACT_CAPABILITY_ID: &str = "mercurio.semantic.impact";
pub const STATE_MACHINE_SIMULATION_CAPABILITY_ID: &str = "mercurio.simulation.state_machine";

pub fn builtin_reasoning_capabilities() -> Vec<CapabilityDescriptor> {
    vec![
        requirement_coverage_capability_descriptor(),
        semantic_impact_capability_descriptor(),
        state_machine_simulation_capability_descriptor(),
    ]
}

pub fn requirement_coverage_capability_descriptor() -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: REQUIREMENT_COVERAGE_CAPABILITY_ID.to_string(),
        kind: CapabilityKind::RequirementCoverage,
        name: "Requirement Coverage".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        api_version: REASONING_API_VERSION.to_string(),
        deterministic: true,
        input_artifact_kinds: vec![
            "runtime_artifact".to_string(),
            "derived_indexes".to_string(),
        ],
        output_artifact_kinds: vec![
            "finding".to_string(),
            "evidence_graph".to_string(),
            "requirement_coverage_summary".to_string(),
        ],
    }
}

pub fn semantic_impact_capability_descriptor() -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: SEMANTIC_IMPACT_CAPABILITY_ID.to_string(),
        kind: CapabilityKind::StaticAnalysis,
        name: "Semantic Impact".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        api_version: REASONING_API_VERSION.to_string(),
        deterministic: true,
        input_artifact_kinds: vec!["runtime_artifact".to_string(), "semantic_graph".to_string()],
        output_artifact_kinds: vec![
            "finding".to_string(),
            "evidence_graph".to_string(),
            "semantic_impact_summary".to_string(),
        ],
    }
}

pub fn state_machine_simulation_capability_descriptor() -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: STATE_MACHINE_SIMULATION_CAPABILITY_ID.to_string(),
        kind: CapabilityKind::Simulation,
        name: "State Machine Simulation".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        api_version: REASONING_API_VERSION.to_string(),
        deterministic: true,
        input_artifact_kinds: vec!["runtime_artifact".to_string(), "semantic_graph".to_string()],
        output_artifact_kinds: vec![
            "finding".to_string(),
            "evidence_graph".to_string(),
            "state_machine_summary".to_string(),
            "state_machine_trace".to_string(),
        ],
    }
}

pub fn analyze_requirement_coverage(
    runtime: &Runtime,
    context: SemanticContextRef,
    request_id: impl Into<String>,
) -> ReasoningReport {
    let view = requirements_table_view(runtime.graph());
    let mut findings = Vec::new();
    let mut evidence_nodes = Vec::new();

    for requirement in &view.rows {
        evidence_nodes.push(requirement_evidence_node(requirement));

        if requirement.satisfied_by.is_empty() {
            findings.push(missing_trace_finding(
                requirement,
                "satisfy",
                "Requirement has no satisfaction evidence",
                "No satisfy relationship reaches this requirement.",
                FindingSeverity::Warning,
            ));
        }

        if requirement.verified_by.is_empty() {
            findings.push(missing_trace_finding(
                requirement,
                "verify",
                "Requirement has no verification evidence",
                "No verify relationship reaches this requirement.",
                FindingSeverity::Error,
            ));
        }
    }

    for warning in &view.warnings {
        findings.push(ReasoningFinding {
            id: "requirement.coverage.no_requirements".to_string(),
            title: "No requirements found".to_string(),
            severity: FindingSeverity::Warning,
            message: warning.clone(),
            elements: Vec::new(),
            source_spans: Vec::new(),
            evidence_ids: Vec::new(),
            properties: BTreeMap::new(),
        });
    }

    let status = if findings.iter().any(|finding| {
        matches!(
            finding.severity,
            FindingSeverity::Error | FindingSeverity::Critical
        )
    }) {
        ReasoningStatus::Failed
    } else if findings.is_empty() {
        ReasoningStatus::Passed
    } else {
        ReasoningStatus::Inconclusive
    };

    let summary_payload = json!({
        "requirementCount": view.rows.len(),
        "satisfiedCount": view.rows.iter().filter(|row| !row.satisfied_by.is_empty()).count(),
        "verifiedCount": view.rows.iter().filter(|row| !row.verified_by.is_empty()).count(),
        "findingCount": findings.len(),
    });

    ReasoningReport {
        request_id: request_id.into(),
        capability: requirement_coverage_capability_descriptor(),
        context,
        status,
        findings,
        artifacts: vec![ReasoningArtifact {
            id: "artifact.requirement_coverage.summary".to_string(),
            kind: "requirement_coverage_summary".to_string(),
            schema: "mercurio.requirement_coverage.summary.v1".to_string(),
            digest: summary_digest(&summary_payload),
            element_refs: view.rows.iter().map(requirement_element_ref).collect(),
            payload: summary_payload,
        }],
        evidence: EvidenceGraph {
            nodes: evidence_nodes,
            edges: Vec::new(),
        },
    }
}

pub fn analyze_semantic_impact(
    runtime: &Runtime,
    context: SemanticContextRef,
    request_id: impl Into<String>,
) -> ReasoningReport {
    let graph = runtime.graph();
    let requirements = requirements_table_view(graph);
    let mut findings = Vec::new();
    let mut evidence_nodes = Vec::new();
    let mut relation_counts = BTreeMap::<String, usize>::new();
    let mut hotspot_count = 0usize;

    for edge in graph.edges() {
        *relation_counts.entry(edge.relation.clone()).or_default() += 1;
    }

    for element in graph.elements() {
        let incoming_count = graph.incoming_edges(element.id).count();
        let outgoing_count = graph.outgoing_edges(element.id).count();
        if incoming_count == 0 && outgoing_count == 0 {
            continue;
        }

        evidence_nodes.push(impact_evidence_node(
            element,
            incoming_count,
            outgoing_count,
        ));

        let degree = incoming_count + outgoing_count;
        if degree >= 5 {
            hotspot_count += 1;
            findings.push(ReasoningFinding {
                id: format!("finding.semantic_impact.hotspot.{}", element.element_id),
                title: "Element has high semantic impact".to_string(),
                severity: FindingSeverity::Info,
                message: format!(
                    "Element participates in {degree} semantic relationships ({incoming_count} incoming, {outgoing_count} outgoing)."
                ),
                elements: vec![element_ref(element)],
                source_spans: Vec::new(),
                evidence_ids: vec![impact_evidence_id(element)],
                properties: BTreeMap::from([
                    ("incomingCount".to_string(), json!(incoming_count)),
                    ("outgoingCount".to_string(), json!(outgoing_count)),
                    ("degree".to_string(), json!(degree)),
                ]),
            });
        }
    }

    for requirement in &requirements.rows {
        if requirement.satisfied_by.is_empty() {
            findings.push(missing_trace_finding(
                requirement,
                "satisfy",
                "Requirement has no downstream satisfaction impact",
                "Impact analysis found no satisfy relationship reaching this requirement.",
                FindingSeverity::Warning,
            ));
        }
        if requirement.verified_by.is_empty() {
            findings.push(missing_trace_finding(
                requirement,
                "verify",
                "Requirement has no downstream verification impact",
                "Impact analysis found no verify relationship reaching this requirement.",
                FindingSeverity::Error,
            ));
        }
    }

    if graph.edges().is_empty() {
        findings.push(ReasoningFinding {
            id: "finding.semantic_impact.no_relationships".to_string(),
            title: "No semantic relationships found".to_string(),
            severity: FindingSeverity::Warning,
            message: "The semantic graph contains no derived relationships to traverse."
                .to_string(),
            elements: Vec::new(),
            source_spans: Vec::new(),
            evidence_ids: Vec::new(),
            properties: BTreeMap::new(),
        });
    }

    let status = if findings.iter().any(|finding| {
        matches!(
            finding.severity,
            FindingSeverity::Error | FindingSeverity::Critical
        )
    }) {
        ReasoningStatus::Failed
    } else if findings.is_empty() {
        ReasoningStatus::Passed
    } else {
        ReasoningStatus::Inconclusive
    };

    let summary_payload = json!({
        "elementCount": graph.elements().len(),
        "relationshipCount": graph.edges().len(),
        "relationCounts": relation_counts,
        "requirementCount": requirements.rows.len(),
        "hotspotCount": hotspot_count,
        "findingCount": findings.len(),
    });

    ReasoningReport {
        request_id: request_id.into(),
        capability: semantic_impact_capability_descriptor(),
        context,
        status,
        findings,
        artifacts: vec![ReasoningArtifact {
            id: "artifact.semantic_impact.summary".to_string(),
            kind: "semantic_impact_summary".to_string(),
            schema: "mercurio.semantic_impact.summary.v1".to_string(),
            digest: summary_digest(&summary_payload),
            element_refs: evidence_nodes
                .iter()
                .flat_map(|node| node.element_refs.clone())
                .collect(),
            payload: summary_payload,
        }],
        evidence: EvidenceGraph {
            nodes: evidence_nodes,
            edges: Vec::new(),
        },
    }
}

pub fn analyze_state_machine_simulation(
    runtime: &Runtime,
    context: SemanticContextRef,
    request_id: impl Into<String>,
) -> ReasoningReport {
    let machines = project_state_machines(runtime);
    let mut findings = Vec::new();
    let mut evidence_nodes = Vec::new();

    if machines.is_empty() {
        findings.push(ReasoningFinding {
            id: "finding.state_machine.no_state_machines".to_string(),
            title: "No state machines found".to_string(),
            severity: FindingSeverity::Warning,
            message: "No KIR state usage elements were found for structural simulation."
                .to_string(),
            elements: Vec::new(),
            source_spans: Vec::new(),
            evidence_ids: Vec::new(),
            properties: BTreeMap::new(),
        });
    }

    let mut trace_payload = Vec::new();
    for machine in &machines {
        evidence_nodes.push(EvidenceNode {
            id: format!("evidence.state_machine.{}", machine.id),
            kind: EvidenceNodeKind::Fact,
            label: machine.label.clone(),
            element_refs: Vec::new(),
            source_spans: Vec::new(),
            properties: BTreeMap::from([
                ("stateCount".to_string(), json!(machine.states.len())),
                (
                    "transitionCount".to_string(),
                    json!(machine.transitions.len()),
                ),
            ]),
        });

        for state in &machine.states {
            evidence_nodes.push(EvidenceNode {
                id: state_evidence_id(state),
                kind: EvidenceNodeKind::KirElement,
                label: state.label.clone(),
                element_refs: vec![SemanticElementRef {
                    element_id: state.id.clone(),
                    qualified_name: None,
                    label: Some(state.label.clone()),
                }],
                source_spans: Vec::new(),
                properties: BTreeMap::from([
                    ("machineId".to_string(), json!(machine.id)),
                    ("isInitial".to_string(), json!(state.is_initial)),
                    ("isFinal".to_string(), json!(state.is_final)),
                ]),
            });
        }

        let initial_states = machine
            .states
            .iter()
            .filter(|state| state.is_initial)
            .collect::<Vec<_>>();
        if machine.states.is_empty() {
            findings.push(machine_finding(
                machine,
                "no_states",
                "State machine has no states",
                "The state machine candidate has no owned states to simulate.",
                FindingSeverity::Warning,
            ));
        }
        if initial_states.is_empty() && !machine.states.is_empty() {
            findings.push(machine_finding(
                machine,
                "no_initial_state",
                "State machine has no initial state",
                "Structural simulation needs one initial state. No state is marked initial.",
                FindingSeverity::Error,
            ));
        }
        if initial_states.len() > 1 {
            findings.push(machine_finding(
                machine,
                "multiple_initial_states",
                "State machine has multiple initial states",
                "Structural simulation found more than one state marked initial.",
                FindingSeverity::Error,
            ));
        }
        for validation in machine.validate_structure() {
            if matches!(
                validation.code.as_str(),
                "missing_parent_state"
                    | "compound_state_missing_initial_child"
                    | "compound_state_multiple_initial_children"
                    | "missing_transition_source"
                    | "missing_transition_target"
            ) {
                findings.push(ReasoningFinding {
                    id: format!(
                        "finding.state_machine.{}.{}",
                        validation.code,
                        validation
                            .state_id
                            .as_deref()
                            .or(validation.transition_id.as_deref())
                            .unwrap_or(machine.id.as_str())
                    ),
                    title: validation
                        .code
                        .replace('_', " ")
                        .split(' ')
                        .map(|word| {
                            let mut chars = word.chars();
                            chars
                                .next()
                                .map(|first| {
                                    first.to_uppercase().collect::<String>() + chars.as_str()
                                })
                                .unwrap_or_default()
                        })
                        .collect::<Vec<_>>()
                        .join(" "),
                    severity: match validation.severity {
                        mercurio_core::StateMachineValidationSeverity::Warning => {
                            FindingSeverity::Warning
                        }
                        mercurio_core::StateMachineValidationSeverity::Error => {
                            FindingSeverity::Error
                        }
                    },
                    message: validation.message,
                    elements: validation
                        .state_id
                        .and_then(|state_id| {
                            machine
                                .states
                                .iter()
                                .find(|state| state.id == state_id)
                                .map(|state| vec![state_ref(state)])
                        })
                        .unwrap_or_default(),
                    source_spans: Vec::new(),
                    evidence_ids: vec![format!("evidence.state_machine.{}", machine.id)],
                    properties: BTreeMap::from([("machineId".to_string(), json!(machine.id))]),
                });
            }
        }

        let reachable = machine.reachable_state_ids();
        for state in &machine.states {
            if !reachable.is_empty() && !reachable.contains(&state.id) {
                findings.push(state_finding(
                    machine,
                    state,
                    "unreachable_state",
                    "State is unreachable",
                    "No transition path reaches this state from the initial state.",
                    FindingSeverity::Warning,
                ));
            }
            let has_outgoing = machine
                .transitions
                .iter()
                .any(|transition| transition.source == state.id);
            if !state.is_final && !has_outgoing {
                findings.push(state_finding(
                    machine,
                    state,
                    "dead_end_state",
                    "State has no outgoing transition",
                    "The state is not final and has no outgoing transition.",
                    FindingSeverity::Warning,
                ));
            }
        }

        for (source, trigger, count) in machine.ambiguous_transition_keys() {
            findings.push(ReasoningFinding {
                id: format!(
                    "finding.state_machine.ambiguous_transition.{}.{}",
                    machine.id,
                    sanitize_identifier(&source)
                ),
                title: "State has ambiguous triggered transitions".to_string(),
                severity: FindingSeverity::Warning,
                message: format!(
                    "State `{source}` has {count} outgoing transitions for trigger `{trigger}`."
                ),
                elements: machine
                    .states
                    .iter()
                    .find(|state| state.id == source)
                    .map(|state| vec![state_ref(state)])
                    .unwrap_or_default(),
                source_spans: Vec::new(),
                evidence_ids: vec![format!("evidence.state_machine.{}", machine.id)],
                properties: BTreeMap::from([
                    ("machineId".to_string(), json!(machine.id)),
                    ("sourceState".to_string(), json!(source)),
                    ("trigger".to_string(), json!(trigger)),
                    ("transitionCount".to_string(), json!(count)),
                ]),
            });
        }

        trace_payload.push(json!({
            "machineId": machine.id,
            "label": machine.label,
            "initialStates": initial_states.iter().map(|state| state.id.clone()).collect::<Vec<_>>(),
            "reachableStates": reachable.iter().cloned().collect::<Vec<_>>(),
            "states": machine.states.iter().map(|state| json!({
                "id": state.id,
                "label": state.label,
                "isInitial": state.is_initial,
                "isFinal": state.is_final,
                "parentState": state.parent_state_id,
            })).collect::<Vec<_>>(),
            "transitions": machine.transitions.iter().map(|transition| json!({
                "id": transition.id,
                "source": transition.source,
                "target": transition.target,
                "trigger": transition.trigger,
                "triggerKind": transition.trigger_kind,
                "guard": transition.guard,
                "effect": transition.effect,
            })).collect::<Vec<_>>(),
        }));
    }

    let status = if findings.iter().any(|finding| {
        matches!(
            finding.severity,
            FindingSeverity::Error | FindingSeverity::Critical
        )
    }) {
        ReasoningStatus::Failed
    } else if findings.is_empty() {
        ReasoningStatus::Passed
    } else {
        ReasoningStatus::Inconclusive
    };

    let state_count = machines
        .iter()
        .map(|machine| machine.states.len())
        .sum::<usize>();
    let transition_count = machines
        .iter()
        .map(|machine| machine.transitions.len())
        .sum::<usize>();
    let summary_payload = json!({
        "machineCount": machines.len(),
        "stateCount": state_count,
        "transitionCount": transition_count,
        "findingCount": findings.len(),
    });
    let trace_payload = json!({
        "schema": "mercurio.state_machine_trace.v1",
        "machines": trace_payload,
    });

    ReasoningReport {
        request_id: request_id.into(),
        capability: state_machine_simulation_capability_descriptor(),
        context,
        status,
        findings,
        artifacts: vec![
            ReasoningArtifact {
                id: "artifact.state_machine.summary".to_string(),
                kind: "state_machine_summary".to_string(),
                schema: "mercurio.state_machine_summary.v1".to_string(),
                digest: summary_digest(&summary_payload),
                element_refs: machines
                    .iter()
                    .flat_map(|machine| machine.states.iter().map(state_ref))
                    .collect(),
                payload: summary_payload,
            },
            ReasoningArtifact {
                id: "artifact.state_machine.trace".to_string(),
                kind: "state_machine_trace".to_string(),
                schema: "mercurio.state_machine_trace.v1".to_string(),
                digest: summary_digest(&trace_payload),
                element_refs: Vec::new(),
                payload: trace_payload,
            },
        ],
        evidence: EvidenceGraph {
            nodes: evidence_nodes,
            edges: Vec::new(),
        },
    }
}

fn impact_evidence_node(
    element: &Element,
    incoming_count: usize,
    outgoing_count: usize,
) -> EvidenceNode {
    EvidenceNode {
        id: impact_evidence_id(element),
        kind: EvidenceNodeKind::KirElement,
        label: element_label(element),
        element_refs: vec![element_ref(element)],
        source_spans: Vec::new(),
        properties: BTreeMap::from([
            ("incomingCount".to_string(), json!(incoming_count)),
            ("outgoingCount".to_string(), json!(outgoing_count)),
        ]),
    }
}

fn machine_finding(
    machine: &StateMachineModel,
    code: &str,
    title: &str,
    message: &str,
    severity: FindingSeverity,
) -> ReasoningFinding {
    ReasoningFinding {
        id: format!("finding.state_machine.{code}.{}", machine.id),
        title: title.to_string(),
        severity,
        message: message.to_string(),
        elements: Vec::new(),
        source_spans: Vec::new(),
        evidence_ids: vec![format!("evidence.state_machine.{}", machine.id)],
        properties: BTreeMap::from([("machineId".to_string(), json!(machine.id))]),
    }
}

fn state_finding(
    machine: &StateMachineModel,
    state: &StateNode,
    code: &str,
    title: &str,
    message: &str,
    severity: FindingSeverity,
) -> ReasoningFinding {
    ReasoningFinding {
        id: format!("finding.state_machine.{code}.{}", state.id),
        title: title.to_string(),
        severity,
        message: message.to_string(),
        elements: vec![state_ref(state)],
        source_spans: Vec::new(),
        evidence_ids: vec![
            format!("evidence.state_machine.{}", machine.id),
            state_evidence_id(state),
        ],
        properties: BTreeMap::from([
            ("machineId".to_string(), json!(machine.id)),
            ("stateId".to_string(), json!(state.id)),
        ]),
    }
}

fn state_ref(state: &StateNode) -> SemanticElementRef {
    SemanticElementRef {
        element_id: state.id.clone(),
        qualified_name: None,
        label: Some(state.label.clone()),
    }
}

fn state_evidence_id(state: &StateNode) -> String {
    format!("evidence.state.{}", state.id)
}

fn impact_evidence_id(element: &Element) -> String {
    format!("evidence.semantic_impact.{}", element.element_id)
}

fn element_ref(element: &Element) -> SemanticElementRef {
    SemanticElementRef {
        element_id: element.element_id.clone(),
        qualified_name: None,
        label: Some(element_label(element)),
    }
}

fn element_label(element: &Element) -> String {
    element
        .properties
        .get("declared_name")
        .or_else(|| element.properties.get("name"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| element.element_id.clone())
}

fn sanitize_identifier(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn missing_trace_finding(
    requirement: &RequirementTableRowDto,
    trace_kind: &str,
    title: &str,
    message: &str,
    severity: FindingSeverity,
) -> ReasoningFinding {
    ReasoningFinding {
        id: format!(
            "finding.requirement.{trace_kind}.missing.{}",
            requirement.id
        ),
        title: title.to_string(),
        severity,
        message: message.to_string(),
        elements: vec![requirement_element_ref(requirement)],
        source_spans: source_spans(requirement),
        evidence_ids: vec![requirement_evidence_id(requirement)],
        properties: BTreeMap::from([
            (
                "requirementId".to_string(),
                Value::String(requirement.id.clone()),
            ),
            (
                "traceKind".to_string(),
                Value::String(trace_kind.to_string()),
            ),
        ]),
    }
}

fn requirement_evidence_node(requirement: &RequirementTableRowDto) -> EvidenceNode {
    EvidenceNode {
        id: requirement_evidence_id(requirement),
        kind: EvidenceNodeKind::KirElement,
        label: requirement
            .name
            .clone()
            .unwrap_or_else(|| requirement.id.clone()),
        element_refs: vec![requirement_element_ref(requirement)],
        source_spans: source_spans(requirement),
        properties: BTreeMap::from([
            (
                "satisfiedBy".to_string(),
                Value::Array(
                    requirement
                        .satisfied_by
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            ),
            (
                "verifiedBy".to_string(),
                Value::Array(
                    requirement
                        .verified_by
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            ),
        ]),
    }
}

fn requirement_element_ref(requirement: &RequirementTableRowDto) -> SemanticElementRef {
    SemanticElementRef {
        element_id: requirement.id.clone(),
        qualified_name: None,
        label: requirement.name.clone(),
    }
}

fn requirement_evidence_id(requirement: &RequirementTableRowDto) -> String {
    format!("evidence.requirement.{}", requirement.id)
}

fn source_spans(requirement: &RequirementTableRowDto) -> Vec<SourceSpanRef> {
    requirement
        .source
        .as_ref()
        .and_then(source_span_ref)
        .into_iter()
        .collect()
}

fn source_span_ref(source: &RequirementSourceDto) -> Option<SourceSpanRef> {
    Some(SourceSpanRef {
        file: source.file.clone()?,
        start_line: u32::try_from(source.start_line?).ok()?,
        start_col: 1,
        end_line: u32::try_from(source.end_line?).ok()?,
        end_col: 1,
    })
}

fn summary_digest(value: &Value) -> String {
    let encoded = serde_json::to_string(value).unwrap_or_default();
    let mut hash = 0xcbf29ce484222325u64;
    for byte in encoded.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64_{hash:016x}")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mercurio_core::{KirDocument, KirElement, Runtime, repo_path};
    use mercurio_reasoner_api::{SemanticArtifactRef, SemanticContextKind, SemanticContextRef};
    use serde_json::{Value, json};

    use super::*;

    #[test]
    fn requirement_coverage_reports_missing_verification() {
        let document =
            KirDocument::from_path(&repo_path("examples/requirements_table_model.json")).unwrap();
        let runtime = Runtime::from_document(document).unwrap();
        let report = analyze_requirement_coverage(&runtime, test_context(), "req-coverage-test");

        assert_eq!(report.status, ReasoningStatus::Failed);
        assert!(report.findings.iter().any(|finding| {
            finding
                .id
                .contains("verify.missing.req.VehicleSafety.DriverAlert")
        }));
        assert_eq!(report.evidence.nodes.len(), 3);
        assert_eq!(
            report.artifacts[0].payload["verifiedCount"],
            serde_json::Value::from(2)
        );
    }

    #[test]
    fn semantic_impact_reports_graph_relationship_summary() {
        let document =
            KirDocument::from_path(&repo_path("examples/requirements_table_model.json")).unwrap();
        let runtime = Runtime::from_document(document).unwrap();
        let report = analyze_semantic_impact(&runtime, test_context(), "semantic-impact-test");

        assert_eq!(report.capability.id, SEMANTIC_IMPACT_CAPABILITY_ID);
        assert_eq!(report.capability.kind, CapabilityKind::StaticAnalysis);
        assert_eq!(report.status, ReasoningStatus::Failed);
        assert!(
            report
                .artifacts
                .iter()
                .any(|artifact| artifact.schema == "mercurio.semantic_impact.summary.v1")
        );
        assert!(
            report.artifacts[0].payload["relationshipCount"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert!(report.findings.iter().any(|finding| {
            finding
                .id
                .contains("verify.missing.req.VehicleSafety.DriverAlert")
        }));
    }

    #[test]
    fn state_machine_simulation_reports_unreachable_and_dead_end_states() {
        let runtime = Runtime::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                state("state.ControllerMode.Off", "ControllerMode", true, false),
                state("state.ControllerMode.On", "ControllerMode", false, false),
                state(
                    "state.ControllerMode.Faulted",
                    "ControllerMode",
                    false,
                    false,
                ),
                transition(
                    "transition.ControllerMode.start",
                    "ControllerMode",
                    "state.ControllerMode.Off",
                    "state.ControllerMode.On",
                    "start",
                ),
            ],
        })
        .unwrap();

        let report =
            analyze_state_machine_simulation(&runtime, test_context(), "state-machine-test");

        assert_eq!(report.capability.id, STATE_MACHINE_SIMULATION_CAPABILITY_ID);
        assert_eq!(report.capability.kind, CapabilityKind::Simulation);
        assert_eq!(report.status, ReasoningStatus::Inconclusive);
        assert_eq!(
            report.artifacts[0].payload["machineCount"],
            serde_json::Value::from(1)
        );
        assert_eq!(
            report.artifacts[1].schema,
            "mercurio.state_machine_trace.v1"
        );
        assert!(report.findings.iter().any(|finding| {
            finding
                .id
                .contains("unreachable_state.state.ControllerMode.Faulted")
        }));
        assert!(report.findings.iter().any(|finding| {
            finding
                .id
                .contains("dead_end_state.state.ControllerMode.On")
        }));
    }

    #[test]
    fn state_machine_simulation_reports_missing_initial_state() {
        let runtime = Runtime::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                state("state.ControllerMode.Off", "ControllerMode", false, false),
                state("state.ControllerMode.On", "ControllerMode", false, false),
            ],
        })
        .unwrap();

        let report =
            analyze_state_machine_simulation(&runtime, test_context(), "state-machine-test");

        assert_eq!(report.status, ReasoningStatus::Failed);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.id.contains("no_initial_state.ControllerMode"))
        );
    }

    fn test_context() -> SemanticContextRef {
        SemanticContextRef {
            context_id: "ctx.test".to_string(),
            kind: SemanticContextKind::Accepted,
            artifact: SemanticArtifactRef {
                artifact_key: "artifact.test".to_string(),
                kir_schema_version: "0.1".to_string(),
                source_authority: Some("test_fixture".to_string()),
                source_revision: None,
            },
        }
    }

    fn state(id: &str, owner: &str, initial: bool, final_state: bool) -> KirElement {
        KirElement {
            id: id.to_string(),
            kind: "StateUsage".to_string(),
            layer: 0,
            properties: BTreeMap::from([
                ("declared_name".to_string(), Value::String(id.to_string())),
                ("owning_type".to_string(), Value::String(owner.to_string())),
                ("is_initial".to_string(), Value::Bool(initial)),
                ("is_final".to_string(), Value::Bool(final_state)),
            ]),
        }
    }

    fn transition(id: &str, owner: &str, source: &str, target: &str, trigger: &str) -> KirElement {
        KirElement {
            id: id.to_string(),
            kind: "TransitionUsage".to_string(),
            layer: 0,
            properties: BTreeMap::from([
                ("owning_type".to_string(), Value::String(owner.to_string())),
                ("source".to_string(), Value::String(source.to_string())),
                ("target".to_string(), Value::String(target.to_string())),
                ("trigger".to_string(), Value::String(trigger.to_string())),
                (
                    "metadata".to_string(),
                    json!({
                        "source": "test"
                    }),
                ),
            ]),
        }
    }
}
