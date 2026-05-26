use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::graph::{Element, Graph};
use crate::runtime::Runtime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateMachineModel {
    pub id: String,
    pub label: String,
    pub states: Vec<StateNode>,
    pub transitions: Vec<TransitionNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateNode {
    pub id: String,
    pub label: String,
    pub owner_id: String,
    pub parent_state_id: Option<String>,
    pub is_initial: bool,
    pub is_final: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionNode {
    pub id: String,
    pub owner_id: String,
    pub source: String,
    pub target: String,
    pub trigger: Option<String>,
    pub trigger_kind: StateTransitionTriggerKind,
    pub guard: Option<Value>,
    pub effect: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateTransitionTriggerKind {
    Event,
    Time,
    After,
    Change,
    Completion,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateMachineValidationFinding {
    pub code: String,
    pub severity: StateMachineValidationSeverity,
    pub message: String,
    pub machine_id: String,
    pub state_id: Option<String>,
    pub transition_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateMachineValidationSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateMachineScenario {
    pub id: String,
    pub initial_state_id: Option<String>,
    pub events: Vec<StateMachineScenarioEvent>,
    pub max_steps: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateMachineScenarioEvent {
    pub id: String,
    pub trigger: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateMachineExecutionReport {
    pub machine_id: String,
    pub status: StateMachineExecutionStatus,
    pub active_configuration: Vec<String>,
    pub steps: Vec<StateMachineTraceStep>,
    pub diagnostics: Vec<StateMachineValidationFinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateMachineExecutionStatus {
    Completed,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateMachineTraceStep {
    pub step: usize,
    pub event_id: Option<String>,
    pub trigger: Option<String>,
    pub transition_id: Option<String>,
    pub before: Vec<String>,
    pub after: Vec<String>,
    pub explanation: String,
}

impl StateMachineModel {
    pub fn reachable_state_ids(&self) -> BTreeSet<String> {
        let mut reachable = BTreeSet::new();
        let mut queue = VecDeque::new();
        for state in self.states.iter().filter(|state| state.is_initial) {
            reachable.insert(state.id.clone());
            queue.push_back(state.id.clone());
        }

        while let Some(state_id) = queue.pop_front() {
            for transition in self
                .transitions
                .iter()
                .filter(|transition| transition.source == state_id)
            {
                if reachable.insert(transition.target.clone()) {
                    queue.push_back(transition.target.clone());
                }
            }
        }

        reachable
    }

    pub fn ambiguous_transition_keys(&self) -> Vec<(String, String, usize)> {
        let mut counts = BTreeMap::<(String, String), usize>::new();
        for transition in &self.transitions {
            let trigger = transition
                .trigger
                .clone()
                .unwrap_or_else(|| "<untriggered>".to_string());
            *counts
                .entry((transition.source.clone(), trigger))
                .or_default() += 1;
        }
        counts
            .into_iter()
            .filter_map(|((source, trigger), count)| {
                (count > 1).then_some((source, trigger, count))
            })
            .collect()
    }

    pub fn validate_structure(&self) -> Vec<StateMachineValidationFinding> {
        let mut findings = Vec::new();
        let states_by_id = self
            .states
            .iter()
            .map(|state| (state.id.as_str(), state))
            .collect::<BTreeMap<_, _>>();

        if self.states.is_empty() {
            findings.push(self.machine_finding(
                "no_states",
                StateMachineValidationSeverity::Warning,
                "The state machine candidate has no owned states to simulate.",
            ));
        }

        let top_initial_states = self
            .states
            .iter()
            .filter(|state| state.parent_state_id.is_none() && state.is_initial)
            .collect::<Vec<_>>();
        if top_initial_states.is_empty() && !self.states.is_empty() {
            findings.push(self.machine_finding(
                "no_initial_state",
                StateMachineValidationSeverity::Error,
                "Structural simulation needs one top-level initial state.",
            ));
        }
        if top_initial_states.len() > 1 {
            findings.push(self.machine_finding(
                "multiple_initial_states",
                StateMachineValidationSeverity::Error,
                "Structural simulation found more than one top-level state marked initial.",
            ));
        }

        for state in &self.states {
            if let Some(parent_id) = &state.parent_state_id
                && !states_by_id.contains_key(parent_id.as_str())
            {
                findings.push(self.state_finding(
                    state,
                    "missing_parent_state",
                    StateMachineValidationSeverity::Error,
                    "Nested state references a parent state that is not present.",
                ));
            }
        }

        for parent in self.states.iter().filter(|candidate| {
            self.states
                .iter()
                .any(|state| state.parent_state_id.as_deref() == Some(candidate.id.as_str()))
        }) {
            let initial_children = self
                .states
                .iter()
                .filter(|state| {
                    state.parent_state_id.as_deref() == Some(parent.id.as_str()) && state.is_initial
                })
                .collect::<Vec<_>>();
            if initial_children.is_empty() {
                findings.push(self.state_finding(
                    parent,
                    "compound_state_missing_initial_child",
                    StateMachineValidationSeverity::Warning,
                    "Compound state has child states but no initial child state.",
                ));
            }
            if initial_children.len() > 1 {
                findings.push(self.state_finding(
                    parent,
                    "compound_state_multiple_initial_children",
                    StateMachineValidationSeverity::Error,
                    "Compound state has more than one initial child state.",
                ));
            }
        }

        for transition in &self.transitions {
            if !states_by_id.contains_key(transition.source.as_str()) {
                findings.push(self.transition_finding(
                    transition,
                    "missing_transition_source",
                    StateMachineValidationSeverity::Error,
                    "Transition source state is not present.",
                ));
            }
            if !states_by_id.contains_key(transition.target.as_str()) {
                findings.push(self.transition_finding(
                    transition,
                    "missing_transition_target",
                    StateMachineValidationSeverity::Error,
                    "Transition target state is not present.",
                ));
            }
        }

        findings
    }

    pub fn execute_scenario(&self, scenario: &StateMachineScenario) -> StateMachineExecutionReport {
        let diagnostics = self.validate_structure();
        if diagnostics
            .iter()
            .any(|finding| finding.severity == StateMachineValidationSeverity::Error)
        {
            return StateMachineExecutionReport {
                machine_id: self.id.clone(),
                status: StateMachineExecutionStatus::Failed,
                active_configuration: Vec::new(),
                steps: Vec::new(),
                diagnostics,
            };
        }

        let mut active = match scenario
            .initial_state_id
            .as_ref()
            .or_else(|| {
                self.states
                    .iter()
                    .find(|state| state.parent_state_id.is_none() && state.is_initial)
                    .map(|state| &state.id)
            })
            .and_then(|state_id| self.enter_state_configuration(state_id))
        {
            Some(configuration) => configuration,
            None => {
                let mut diagnostics = diagnostics;
                diagnostics.push(self.machine_finding(
                    "no_executable_initial_state",
                    StateMachineValidationSeverity::Error,
                    "No initial state configuration could be created.",
                ));
                return StateMachineExecutionReport {
                    machine_id: self.id.clone(),
                    status: StateMachineExecutionStatus::Failed,
                    active_configuration: Vec::new(),
                    steps: Vec::new(),
                    diagnostics,
                };
            }
        };

        let mut steps = Vec::new();
        for (index, event) in scenario
            .events
            .iter()
            .take(scenario.max_steps.max(1))
            .enumerate()
        {
            let before = active.clone();
            if let Some(transition) = self.select_transition(&active, &event.trigger) {
                if let Some(after) = self.enter_state_configuration(&transition.target) {
                    active = after;
                    steps.push(StateMachineTraceStep {
                        step: index + 1,
                        event_id: Some(event.id.clone()),
                        trigger: Some(event.trigger.clone()),
                        transition_id: Some(transition.id.clone()),
                        before,
                        after: active.clone(),
                        explanation: format!(
                            "Transition `{}` fired for trigger `{}`.",
                            transition.id, event.trigger
                        ),
                    });
                } else {
                    steps.push(StateMachineTraceStep {
                        step: index + 1,
                        event_id: Some(event.id.clone()),
                        trigger: Some(event.trigger.clone()),
                        transition_id: Some(transition.id.clone()),
                        before: before.clone(),
                        after: before,
                        explanation: format!(
                            "Transition `{}` targeted a state that could not be entered.",
                            transition.id
                        ),
                    });
                    return StateMachineExecutionReport {
                        machine_id: self.id.clone(),
                        status: StateMachineExecutionStatus::Failed,
                        active_configuration: active,
                        steps,
                        diagnostics,
                    };
                }
            } else {
                steps.push(StateMachineTraceStep {
                    step: index + 1,
                    event_id: Some(event.id.clone()),
                    trigger: Some(event.trigger.clone()),
                    transition_id: None,
                    before: before.clone(),
                    after: before,
                    explanation: format!(
                        "No enabled transition matched trigger `{}`.",
                        event.trigger
                    ),
                });
                return StateMachineExecutionReport {
                    machine_id: self.id.clone(),
                    status: StateMachineExecutionStatus::Blocked,
                    active_configuration: active,
                    steps,
                    diagnostics,
                };
            }
        }

        StateMachineExecutionReport {
            machine_id: self.id.clone(),
            status: StateMachineExecutionStatus::Completed,
            active_configuration: active,
            steps,
            diagnostics,
        }
    }

    fn enter_state_configuration(&self, state_id: &str) -> Option<Vec<String>> {
        let mut configuration = self.ancestor_path(state_id)?;
        let mut current = state_id.to_string();
        loop {
            let Some(initial_child) = self.states.iter().find(|state| {
                state.parent_state_id.as_deref() == Some(current.as_str()) && state.is_initial
            }) else {
                return Some(configuration);
            };
            configuration.push(initial_child.id.clone());
            current = initial_child.id.clone();
        }
    }

    fn ancestor_path(&self, state_id: &str) -> Option<Vec<String>> {
        let mut path = Vec::new();
        let mut cursor = self.states.iter().find(|state| state.id == state_id)?;
        loop {
            path.push(cursor.id.clone());
            let Some(parent_id) = &cursor.parent_state_id else {
                path.reverse();
                return Some(path);
            };
            cursor = self.states.iter().find(|state| state.id == *parent_id)?;
        }
    }

    fn select_transition<'a>(
        &'a self,
        active_configuration: &[String],
        trigger: &str,
    ) -> Option<&'a TransitionNode> {
        active_configuration.iter().rev().find_map(|state_id| {
            self.transitions.iter().find(|transition| {
                transition.source == *state_id
                    && transition.trigger.as_deref() == Some(trigger)
                    && transition.guard.is_none()
            })
        })
    }

    fn machine_finding(
        &self,
        code: &str,
        severity: StateMachineValidationSeverity,
        message: &str,
    ) -> StateMachineValidationFinding {
        StateMachineValidationFinding {
            code: code.to_string(),
            severity,
            message: message.to_string(),
            machine_id: self.id.clone(),
            state_id: None,
            transition_id: None,
        }
    }

    fn state_finding(
        &self,
        state: &StateNode,
        code: &str,
        severity: StateMachineValidationSeverity,
        message: &str,
    ) -> StateMachineValidationFinding {
        StateMachineValidationFinding {
            code: code.to_string(),
            severity,
            message: message.to_string(),
            machine_id: self.id.clone(),
            state_id: Some(state.id.clone()),
            transition_id: None,
        }
    }

    fn transition_finding(
        &self,
        transition: &TransitionNode,
        code: &str,
        severity: StateMachineValidationSeverity,
        message: &str,
    ) -> StateMachineValidationFinding {
        StateMachineValidationFinding {
            code: code.to_string(),
            severity,
            message: message.to_string(),
            machine_id: self.id.clone(),
            state_id: None,
            transition_id: Some(transition.id.clone()),
        }
    }
}

pub fn project_state_machines(runtime: &Runtime) -> Vec<StateMachineModel> {
    project_state_machines_from_graph(runtime.graph())
}

pub fn project_state_machines_from_graph(graph: &Graph) -> Vec<StateMachineModel> {
    let mut states_by_owner = BTreeMap::<String, Vec<StateNode>>::new();
    let mut transitions_by_owner = BTreeMap::<String, Vec<TransitionNode>>::new();

    for element in graph.elements() {
        if is_state_element(element) {
            let owner = owner_id(element).unwrap_or_else(|| "state_machine.root".to_string());
            states_by_owner
                .entry(owner.clone())
                .or_default()
                .push(StateNode {
                    id: element.element_id.clone(),
                    label: element_label(element),
                    owner_id: owner,
                    parent_state_id: parent_state_id(element),
                    is_initial: bool_property(element, &["is_initial", "initial"])
                        || string_property_any(element, &["purpose", "state_kind", "kind_role"])
                            .is_some_and(|value| value.eq_ignore_ascii_case("initial")),
                    is_final: bool_property(element, &["is_final", "final"])
                        || string_property_any(element, &["purpose", "state_kind", "kind_role"])
                            .is_some_and(|value| value.eq_ignore_ascii_case("final")),
                });
            continue;
        }

        if is_transition_element(element)
            && let (Some(source), Some(target)) = (
                string_property_any(element, &["source", "source_state", "from"]),
                string_property_any(element, &["target", "target_state", "to"]),
            )
        {
            let owner = owner_id(element).unwrap_or_else(|| {
                source
                    .rsplit_once(['.', ':', '/'])
                    .map(|(prefix, _)| prefix.to_string())
                    .unwrap_or_else(|| "state_machine.root".to_string())
            });
            transitions_by_owner
                .entry(owner.clone())
                .or_default()
                .push(TransitionNode {
                    id: element.element_id.clone(),
                    owner_id: owner,
                    source,
                    target,
                    trigger: string_property_any(element, &["trigger", "event", "guard"]),
                    trigger_kind: transition_trigger_kind(element),
                    guard: element.properties.get("guard").cloned(),
                    effect: string_property_any(element, &["effect", "effect_action"]),
                });
        }
    }

    let mut owners = states_by_owner.keys().cloned().collect::<BTreeSet<_>>();
    owners.extend(transitions_by_owner.keys().cloned());

    owners
        .into_iter()
        .map(|owner| {
            let mut states = states_by_owner.remove(&owner).unwrap_or_default();
            states.sort_by(|left, right| left.id.cmp(&right.id));
            let mut transitions = transitions_by_owner.remove(&owner).unwrap_or_default();
            transitions.sort_by(|left, right| left.id.cmp(&right.id));
            StateMachineModel {
                label: owner
                    .rsplit(['.', ':', '/'])
                    .find(|part| !part.is_empty())
                    .unwrap_or(&owner)
                    .to_string(),
                id: owner,
                states,
                transitions,
            }
        })
        .collect()
}

fn is_state_element(element: &Element) -> bool {
    let kind = element.kind.to_ascii_lowercase();
    kind.contains("stateusage")
        || kind.contains("stateaction")
        || string_property_any(element, &["type", "definition"])
            .is_some_and(|value| value.contains("States::StateAction"))
}

fn is_transition_element(element: &Element) -> bool {
    let kind = element.kind.to_ascii_lowercase();
    kind.contains("transition")
        || kind.contains("succession")
        || element.element_id.starts_with("transition.")
}

fn owner_id(element: &Element) -> Option<String> {
    string_property_any(
        element,
        &[
            "owner",
            "owning_type",
            "owning_definition",
            "owning_namespace",
        ],
    )
}

fn parent_state_id(element: &Element) -> Option<String> {
    string_property_any(
        element,
        &[
            "parent_state",
            "parentState",
            "owning_state",
            "owningState",
            "enclosing_state",
            "enclosingState",
        ],
    )
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

fn string_property_any(element: &Element, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        element
            .properties
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn bool_property(element: &Element, keys: &[&str]) -> bool {
    keys.iter().any(|key| {
        element
            .properties
            .get(*key)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    })
}

fn transition_trigger_kind(element: &Element) -> StateTransitionTriggerKind {
    match string_property_any(element, &["trigger_kind", "triggerKind"])
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("event") => StateTransitionTriggerKind::Event,
        Some("time") | Some("at") => StateTransitionTriggerKind::Time,
        Some("after") | Some("duration") => StateTransitionTriggerKind::After,
        Some("change") | Some("when") => StateTransitionTriggerKind::Change,
        Some("completion") | Some("then") => StateTransitionTriggerKind::Completion,
        _ => {
            if string_property_any(element, &["trigger", "event"]).is_some() {
                StateTransitionTriggerKind::Event
            } else {
                StateTransitionTriggerKind::Unknown
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;

    use crate::{KirDocument, KirElement, Runtime};

    use super::*;

    #[test]
    fn projects_flat_state_machine() {
        let runtime = Runtime::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                state("state.ControllerMode.Off", "ControllerMode", true, false),
                state("state.ControllerMode.On", "ControllerMode", false, false),
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

        let machines = project_state_machines(&runtime);

        assert_eq!(machines.len(), 1);
        assert_eq!(machines[0].id, "ControllerMode");
        assert_eq!(machines[0].states.len(), 2);
        assert_eq!(machines[0].transitions.len(), 1);
        assert!(
            machines[0]
                .reachable_state_ids()
                .contains("state.ControllerMode.On")
        );
    }

    #[test]
    fn preserves_nested_state_parent_id() {
        let runtime = Runtime::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                state("state.Server.Active", "ServerBehavior", true, false),
                nested_state(
                    "state.Server.Active.Waiting",
                    "ServerBehavior",
                    "state.Server.Active",
                    false,
                    false,
                ),
            ],
        })
        .unwrap();

        let machines = project_state_machines(&runtime);
        let nested = machines[0]
            .states
            .iter()
            .find(|state| state.id == "state.Server.Active.Waiting")
            .unwrap();

        assert_eq!(
            nested.parent_state_id.as_deref(),
            Some("state.Server.Active")
        );
    }

    #[test]
    fn executes_nested_state_transition_from_active_leaf() {
        let runtime = Runtime::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                state("state.Server.Active", "ServerBehavior", true, false),
                nested_state(
                    "state.Server.Active.Waiting",
                    "ServerBehavior",
                    "state.Server.Active",
                    true,
                    false,
                ),
                state("state.Server.Off", "ServerBehavior", false, false),
                transition(
                    "transition.Server.stop",
                    "ServerBehavior",
                    "state.Server.Active.Waiting",
                    "state.Server.Off",
                    "stop",
                ),
            ],
        })
        .unwrap();

        let machines = project_state_machines(&runtime);
        let report = machines[0].execute_scenario(&StateMachineScenario {
            id: "scenario.stop".to_string(),
            initial_state_id: None,
            events: vec![StateMachineScenarioEvent {
                id: "event.stop".to_string(),
                trigger: "stop".to_string(),
            }],
            max_steps: 8,
        });

        assert_eq!(report.status, StateMachineExecutionStatus::Completed);
        assert_eq!(report.active_configuration, vec!["state.Server.Off"]);
        assert_eq!(
            report.steps[0].transition_id.as_deref(),
            Some("transition.Server.stop")
        );
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

    fn nested_state(
        id: &str,
        owner: &str,
        parent: &str,
        initial: bool,
        final_state: bool,
    ) -> KirElement {
        let mut element = state(id, owner, initial, final_state);
        element.properties.insert(
            "parent_state".to_string(),
            Value::String(parent.to_string()),
        );
        element
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
                    "trigger_kind".to_string(),
                    Value::String("event".to_string()),
                ),
            ]),
        }
    }
}
