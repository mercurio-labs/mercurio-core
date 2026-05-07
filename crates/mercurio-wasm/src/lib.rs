use std::collections::BTreeMap;

use mercurio_core::diagrams::DiagramError;
use mercurio_core::frontend::ast::{Declaration, PartUsageDecl, SourceSpan};
use mercurio_core::frontend::sysml::parse_sysml_recovering;
use mercurio_core::{
    AssessmentAssertion, AssessmentExpectation, AssessmentQuery, AssessmentSpec, AssessmentStatus,
    Atom, DiagramRenderRequestDto, ExecutionContext, Graph, KirDocument,
    MetamodelAttributeRegistry, Runtime, SourceLanguage, Term, compile_kerml_text,
    compile_sysml_text_with_context_report, evaluate, format_text, lint_text, list_diagram_kinds,
    load_default_rulepacks, parse_kerml, render_diagram, requirements_table_view,
    run_evaluation_assessment, run_graph_assessment, sysml_module_assessment_facts,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

const DEFAULT_STDLIB: &str = include_str!("../../../resources/stdlib.kir.json");

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen(js_name = version)]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[wasm_bindgen(js_name = compileSysml)]
pub fn compile_sysml(input: &str, options: JsValue) -> JsValue {
    json_response(|| {
        let options = CompileOptions::from_js(options)?;
        let stdlib = load_stdlib(options.stdlib)?;
        let report =
            compile_sysml_text_with_context_report(input, &options.source_name, &[], &stdlib);
        let value = json!({
            "status": semantic_status(report.status),
            "document": report.document,
        });
        Ok(Response {
            ok: report.document.is_some() && report.diagnostics.is_empty(),
            value: Some(value),
            diagnostics: serde_json::to_value(report.diagnostics)?,
            errors: Vec::new(),
            metadata: metadata([
                ("sourceName", json!(options.source_name)),
                ("language", json!("sysml")),
            ]),
        })
    })
}

#[wasm_bindgen(js_name = compileKerml)]
pub fn compile_kerml(input: &str, options: JsValue) -> JsValue {
    json_response(|| {
        let options = CompileOptions::from_js(options)?;
        let stdlib = load_stdlib(options.stdlib)?;
        match compile_kerml_text(input, &options.source_name, &stdlib) {
            Ok(document) => Ok(success(
                json!({ "status": "ok", "document": document }),
                [
                    ("sourceName", json!(options.source_name)),
                    ("language", json!("kerml")),
                ],
            )),
            Err(error) => Ok(error_response(
                "compile",
                error.to_string(),
                Some(serde_json::to_value(vec![error])?),
            )),
        }
    })
}

#[wasm_bindgen(js_name = lint)]
pub fn lint(input: &str, language: &str, options: JsValue) -> JsValue {
    json_response(|| {
        let options = CompileOptions::from_js(options)?;
        let language = parse_language(language)?;
        let stdlib = load_stdlib(options.stdlib)?;
        let report = lint_text(input, &options.source_name, language, &[], &stdlib);
        Ok(Response {
            ok: !report.has_errors(),
            value: Some(serde_json::to_value(report)?),
            diagnostics: json!([]),
            errors: Vec::new(),
            metadata: metadata([("sourceName", json!(options.source_name))]),
        })
    })
}

#[wasm_bindgen(js_name = formatText)]
pub fn format_source(input: &str, language: &str) -> JsValue {
    json_response(|| {
        let language = parse_language(language)?;
        let formatted = format_text(input, language)?;
        Ok(success(
            json!({ "text": formatted }),
            [("language", json!(language.as_str()))],
        ))
    })
}

#[wasm_bindgen(js_name = listDiagramKinds)]
pub fn wasm_list_diagram_kinds() -> JsValue {
    json_response(|| Ok(success(serde_json::to_value(list_diagram_kinds())?, [])))
}

#[wasm_bindgen(js_name = renderDiagram)]
pub fn wasm_render_diagram(document: JsValue, request: JsValue) -> JsValue {
    json_response(|| {
        let document: KirDocument = from_js(document)?;
        let request: DiagramRenderRequestDto = from_js(request)?;
        let graph = Graph::from_document(document)?;
        let registry = MetamodelAttributeRegistry::build(&graph);
        let view = render_diagram(&graph, &registry, request.spec)?;
        Ok(success(serde_json::to_value(view)?, []))
    })
}

#[wasm_bindgen(js_name = requirementsTable)]
pub fn wasm_requirements_table(document: JsValue) -> JsValue {
    json_response(|| {
        let document: KirDocument = from_js(document)?;
        let runtime = Runtime::from_document(document)?;
        Ok(success(
            serde_json::to_value(requirements_table_view(runtime.graph()))?,
            [],
        ))
    })
}

#[wasm_bindgen(js_name = queryRuntime)]
pub fn wasm_query_runtime(document: JsValue, query: JsValue) -> JsValue {
    json_response(|| {
        let document: KirDocument = from_js(document)?;
        let query: RuntimeQuery = from_js(query)?;
        let runtime = Runtime::from_document(document)?;
        Ok(success(run_runtime_query(&runtime, query)?, []))
    })
}

#[wasm_bindgen(js_name = runAssessment)]
pub fn wasm_run_assessment(document: JsValue, spec: JsValue) -> JsValue {
    json_response(|| {
        let document: KirDocument = from_js(document)?;
        let spec: AssessmentSpec = from_js(spec)?;
        let graph = Graph::from_document(document)?;
        let rulepacks = load_default_rulepacks()?;
        let report = run_graph_assessment(&graph, &rulepacks, &spec)?;
        Ok(success(serde_json::to_value(report)?, []))
    })
}

#[wasm_bindgen(js_name = runTrainingAssessment)]
pub fn wasm_run_training_assessment(input: &str, request: JsValue) -> JsValue {
    json_response(|| {
        let request: TrainingAssessmentRequest = from_js(request)?;
        match request.assessment_id.as_str() {
            "package-structure.two-packages" => {
                run_package_structure_training_assessment(input, request)
            }
            "part-usage.rotors-six" => run_rotors_multiplicity_training_assessment(input, request),
            "interface-connection.command-link" => {
                run_command_link_training_assessment(input, request)
            }
            _ => Ok(success(
                json!({
                    "assessmentId": request.assessment_id,
                    "status": "failed",
                    "command": "mercurio assess",
                    "transcript": ["unsupported assessment id"],
                    "facts": {
                        "packages": [],
                        "packageCount": 0,
                        "expectedPackageCount": request.expected_package_count,
                    },
                    "diagnostics": [],
                }),
                [("runtime", json!("wasm"))],
            )),
        }
    })
}

#[wasm_bindgen(js_name = parseSysmlSnippet)]
pub fn wasm_parse_sysml_snippet(input: &str, request: JsValue) -> JsValue {
    json_response(|| {
        let request: SnippetParseRequest = from_js(request)?;
        let parse_report = match parse_sysml_recovering(input) {
            Ok(report) => report,
            Err(diagnostic) => {
                return Ok(success(
                    json!({
                        "diagnostics": [snippet_diagnostic(&diagnostic)],
                        "symbols": [],
                        "outline": [],
                    }),
                    [("runtime", json!("wasm")), ("sourceName", json!(request.path))],
                ));
            }
        };
        if !parse_report.diagnostics.is_empty() {
            return Ok(success(
                json!({
                    "diagnostics": parse_report.diagnostics.iter().map(snippet_diagnostic).collect::<Vec<_>>(),
                    "symbols": [],
                    "outline": [],
                }),
                [("runtime", json!("wasm")), ("sourceName", json!(request.path))],
            ));
        }

        let mut symbols = Vec::new();
        let mut outline = Vec::new();
        if parse_report.module.members.is_empty() {
            if let Some(package) = &parse_report.module.package {
                let id = package.name.as_colon_string();
                outline.push(package_outline_node(&id, &id, &package.span, &package.members, &mut symbols));
            }
        } else {
            for declaration in &parse_report.module.members {
                outline.push(declaration_outline_node(declaration, None, &mut symbols));
            }
        }
        let stdlib = load_stdlib(None)?;
        let semantic_report =
            compile_sysml_text_with_context_report(input, &request.path, &[], &stdlib);
        let diagnostics = semantic_report
            .diagnostics
            .iter()
            .map(snippet_diagnostic)
            .collect::<Vec<_>>();

        Ok(success(
            json!({
                "diagnostics": diagnostics,
                "symbols": symbols,
                "outline": outline,
            }),
            [("runtime", json!("wasm")), ("sourceName", json!(request.path))],
        ))
    })
}

#[wasm_bindgen(js_name = MercurioSession)]
pub struct MercurioSession {
    stdlib: KirDocument,
    sources: Vec<SessionSource>,
}

#[wasm_bindgen(js_class = MercurioSession)]
impl MercurioSession {
    #[wasm_bindgen(constructor)]
    pub fn new(options: JsValue) -> Result<MercurioSession, JsValue> {
        let options = CompileOptions::from_js(options).map_err(js_error)?;
        let stdlib = load_stdlib(options.stdlib).map_err(js_error)?;
        Ok(Self {
            stdlib,
            sources: Vec::new(),
        })
    }

    #[wasm_bindgen(js_name = addSource)]
    pub fn add_source(&mut self, language: &str, source_name: &str, input: &str) -> JsValue {
        json_response(|| {
            let language = parse_language(language)?;
            let context = self
                .sources
                .iter()
                .map(|source| source.module.clone())
                .collect::<Vec<_>>();
            let module = match language {
                SourceLanguage::Sysml => parse_sysml_recovering(input)?.module,
                SourceLanguage::Kerml => parse_kerml(input)?,
            };
            let document = match language {
                SourceLanguage::Sysml => compile_sysml_text_with_context_report(
                    input,
                    source_name,
                    &context,
                    &self.stdlib,
                )
                .document
                .ok_or_else(|| WasmError::new("compile", "SysML compilation failed"))?,
                SourceLanguage::Kerml => compile_kerml_text(input, source_name, &self.stdlib)?,
            };
            self.sources.push(SessionSource {
                source_name: source_name.to_string(),
                language,
                module,
                document,
            });
            Ok(success(
                json!({ "sourceName": source_name, "sourceCount": self.sources.len() }),
                [("language", json!(language.as_str()))],
            ))
        })
    }

    #[wasm_bindgen(js_name = clear)]
    pub fn clear(&mut self) {
        self.sources.clear();
    }

    #[wasm_bindgen(js_name = document)]
    pub fn document(&self) -> JsValue {
        json_response(|| {
            let document = self.merged_document()?;
            Ok(success(
                serde_json::to_value(document)?,
                [("sourceCount", json!(self.sources.len()))],
            ))
        })
    }

    #[wasm_bindgen(js_name = renderDiagram)]
    pub fn render_diagram(&self, request: JsValue) -> JsValue {
        json_response(|| {
            let request: DiagramRenderRequestDto = from_js(request)?;
            let graph = self.graph()?;
            let registry = MetamodelAttributeRegistry::build(&graph);
            let view = render_diagram(&graph, &registry, request.spec)?;
            Ok(success(serde_json::to_value(view)?, []))
        })
    }

    #[wasm_bindgen(js_name = requirementsTable)]
    pub fn requirements_table(&self) -> JsValue {
        json_response(|| {
            let runtime = Runtime::from_document(self.merged_document()?)?;
            Ok(success(
                serde_json::to_value(requirements_table_view(runtime.graph()))?,
                [],
            ))
        })
    }

    #[wasm_bindgen(js_name = queryRuntime)]
    pub fn query_runtime(&self, query: JsValue) -> JsValue {
        json_response(|| {
            let query: RuntimeQuery = from_js(query)?;
            let runtime = Runtime::from_document(self.merged_document()?)?;
            Ok(success(run_runtime_query(&runtime, query)?, []))
        })
    }

    #[wasm_bindgen(js_name = runAssessment)]
    pub fn run_assessment(&self, spec: JsValue) -> JsValue {
        json_response(|| {
            let spec: AssessmentSpec = from_js(spec)?;
            let graph = self.graph()?;
            let rulepacks = load_default_rulepacks()?;
            let report = run_graph_assessment(&graph, &rulepacks, &spec)?;
            Ok(success(serde_json::to_value(report)?, []))
        })
    }
}

impl MercurioSession {
    fn merged_document(&self) -> Result<KirDocument, WasmError> {
        let mut elements = self.stdlib.elements.clone();
        for source in &self.sources {
            elements.extend(source.document.elements.clone());
        }
        let document = KirDocument {
            metadata: BTreeMap::from([
                ("source_count".to_string(), json!(self.sources.len())),
                (
                    "sources".to_string(),
                    json!(
                        self.sources
                            .iter()
                            .map(|source| json!({
                                "sourceName": source.source_name,
                                "language": source.language.as_str(),
                                "elementCount": source.document.elements.len(),
                            }))
                            .collect::<Vec<_>>()
                    ),
                ),
            ]),
            elements,
        };
        document.validate()?;
        Ok(document)
    }

    fn graph(&self) -> Result<Graph, WasmError> {
        Ok(Graph::from_document(self.merged_document()?)?)
    }
}

struct SessionSource {
    source_name: String,
    language: SourceLanguage,
    module: mercurio_core::frontend::ast::SysmlModule,
    document: KirDocument,
}

#[derive(Default)]
struct CompileOptions {
    source_name: String,
    stdlib: Option<KirDocument>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeQuery {
    kind: RuntimeQueryKind,
    #[serde(default)]
    type_id: Option<String>,
    #[serde(default)]
    feature_id: Option<String>,
    #[serde(default)]
    owner_id: Option<String>,
    #[serde(default)]
    context: RuntimeContextDto,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum RuntimeQueryKind {
    Subtypes,
    Features,
    Evaluate,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeContextDto {
    #[serde(default)]
    version: u64,
    #[serde(default)]
    values: Vec<RuntimeValueDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeValueDto {
    owner_id: String,
    feature_id: String,
    value: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrainingAssessmentRequest {
    assessment_id: String,
    #[serde(default = "default_source_name")]
    filename: String,
    #[serde(default)]
    expected_package_count: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnippetParseRequest {
    #[serde(default = "default_source_name")]
    path: String,
}

impl CompileOptions {
    fn from_js(value: JsValue) -> Result<Self, WasmError> {
        if value.is_null() || value.is_undefined() {
            return Ok(Self {
                source_name: "memory.sysml".to_string(),
                stdlib: None,
            });
        }
        let raw: Value = from_js(value)?;
        let source_name = raw
            .get("sourceName")
            .or_else(|| raw.get("source_name"))
            .and_then(Value::as_str)
            .unwrap_or("memory.sysml")
            .to_string();
        let stdlib = raw
            .get("stdlib")
            .cloned()
            .map(serde_json::from_value)
            .transpose()?;
        Ok(Self {
            source_name,
            stdlib,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    ok: bool,
    value: Option<Value>,
    diagnostics: Value,
    errors: Vec<WasmError>,
    metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WasmError {
    code: String,
    message: String,
}

impl WasmError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for WasmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for WasmError {}

impl From<serde_json::Error> for WasmError {
    fn from(value: serde_json::Error) -> Self {
        Self::new("json", value.to_string())
    }
}

macro_rules! impl_error {
    ($source:ty, $code:literal) => {
        impl From<$source> for WasmError {
            fn from(value: $source) -> Self {
                Self::new($code, value.to_string())
            }
        }
    };
}

impl_error!(mercurio_core::DatalogError, "datalog");
impl_error!(mercurio_core::GraphError, "graph");
impl_error!(mercurio_core::KirError, "kir");
impl_error!(mercurio_core::RuntimeError, "runtime");
impl_error!(mercurio_core::AssessmentError, "assessment");
impl_error!(DiagramError, "diagram");
impl_error!(mercurio_core::FormatError, "format");
impl_error!(
    mercurio_core::frontend::diagnostics::Diagnostic,
    "diagnostic"
);

fn load_stdlib(stdlib: Option<KirDocument>) -> Result<KirDocument, WasmError> {
    match stdlib {
        Some(document) => {
            document.validate()?;
            Ok(document)
        }
        None => KirDocument::from_str(DEFAULT_STDLIB).map_err(Into::into),
    }
}

fn parse_language(language: &str) -> Result<SourceLanguage, WasmError> {
    match language.to_ascii_lowercase().as_str() {
        "sysml" | "sysml2" => Ok(SourceLanguage::Sysml),
        "kerml" => Ok(SourceLanguage::Kerml),
        _ => Err(WasmError::new(
            "language",
            format!("unsupported source language: {language}"),
        )),
    }
}

fn semantic_status(status: mercurio_core::SemanticCompileStatus) -> &'static str {
    match status {
        mercurio_core::SemanticCompileStatus::Ok => "ok",
        mercurio_core::SemanticCompileStatus::Partial => "partial",
        mercurio_core::SemanticCompileStatus::Failed => "failed",
    }
}

fn run_package_structure_training_assessment(
    input: &str,
    request: TrainingAssessmentRequest,
) -> Result<Response, WasmError> {
    let expected_package_count = request.expected_package_count.unwrap_or(2);
    let command = format!(
        "mercurio assess {} --check package-structure --expect packages={expected_package_count}",
        request.filename
    );
    let parse_report = match parse_sysml_recovering(input) {
        Ok(report) => report,
        Err(diagnostic) => {
            return Ok(success(
                json!({
                    "assessmentId": request.assessment_id,
                    "status": "failed",
                    "command": command,
                    "transcript": [
                        "checking package structure...",
                        "parsing source...",
                        format!("parse error: {}", diagnostic.message),
                        "result: failed",
                    ],
                    "facts": {
                        "packages": [],
                        "packageCount": 0,
                        "expectedPackageCount": expected_package_count,
                    },
                    "diagnostics": [snippet_diagnostic(&diagnostic)],
                }),
                [("runtime", json!("wasm"))],
            ));
        }
    };

    let diagnostics = parse_report
        .diagnostics
        .iter()
        .map(snippet_diagnostic)
        .collect::<Vec<_>>();
    let facts = sysml_module_assessment_facts(&parse_report.module);
    let evaluation = evaluate(facts, &[])?;
    let spec = package_structure_assessment_spec(expected_package_count);
    let report = run_evaluation_assessment(&evaluation, &spec)?;
    let packages = package_bindings(&evaluation);
    let package_count = packages.len();
    let passed = diagnostics.is_empty() && report.status == AssessmentStatus::Pass;
    let mut transcript = vec![
        "checking package structure...".to_string(),
        "parsing source...".to_string(),
        "building assessment fact base...".to_string(),
        format!("running core assessment `{}`...", report.id),
        "found packages:".to_string(),
    ];
    if packages.is_empty() {
        transcript.push("  none".to_string());
    } else {
        transcript.extend(packages.iter().map(|package| format!("  - {package}")));
    }
    transcript.push(format!(
        "expected packages: {expected_package_count}; actual packages: {package_count}"
    ));
    if !diagnostics.is_empty() {
        transcript.push(format!("diagnostics: {}", diagnostics.len()));
    }
    for assertion in &report.assertions {
        transcript.push(format!(
            "assert {}: {}",
            assertion.id,
            match assertion.status {
                AssessmentStatus::Pass => "pass",
                AssessmentStatus::Failed => "failed",
            }
        ));
    }
    transcript.push(format!("result: {}", if passed { "pass" } else { "failed" }));

    Ok(success(
        json!({
            "assessmentId": request.assessment_id,
            "status": if passed { "pass" } else { "failed" },
            "command": command,
            "transcript": transcript,
            "facts": {
                "packages": packages,
                "packageCount": package_count,
                "expectedPackageCount": expected_package_count,
            },
            "diagnostics": diagnostics,
        }),
        [("runtime", json!("wasm"))],
    ))
}

fn run_rotors_multiplicity_training_assessment(
    input: &str,
    request: TrainingAssessmentRequest,
) -> Result<Response, WasmError> {
    let command = format!(
        "mercurio assess {} --check part-usage --expect rotors.multiplicity=6",
        request.filename
    );
    let parse_report = match parse_sysml_recovering(input) {
        Ok(report) => report,
        Err(diagnostic) => {
            return Ok(success(
                json!({
                    "assessmentId": request.assessment_id,
                    "status": "failed",
                    "command": command,
                    "transcript": [
                        "checking rotor part usage...",
                        "parsing source...",
                        format!("parse error: {}", diagnostic.message),
                        "result: failed",
                    ],
                    "facts": {
                        "partUsageFound": false,
                        "typeFound": false,
                        "multiplicity": null,
                        "expectedMultiplicity": "6",
                    },
                    "diagnostics": [snippet_diagnostic(&diagnostic)],
                }),
                [("runtime", json!("wasm"))],
            ));
        }
    };

    let diagnostics = parse_report
        .diagnostics
        .iter()
        .map(snippet_diagnostic)
        .collect::<Vec<_>>();
    let facts = sysml_module_assessment_facts(&parse_report.module);
    let evaluation = evaluate(facts, &[])?;
    let spec = rotors_multiplicity_assessment_spec();
    let report = run_evaluation_assessment(&evaluation, &spec)?;
    let passed = diagnostics.is_empty() && report.status == AssessmentStatus::Pass;
    let rotor_usage_id = named_part_usage_id(&evaluation, "rotors");
    let found_type = rotor_usage_id
        .as_deref()
        .and_then(|usage_id| fact_value(&evaluation, "type", usage_id))
        .unwrap_or_else(|| "not found".to_string());
    let found_multiplicity = rotor_usage_id
        .as_deref()
        .and_then(|usage_id| fact_value(&evaluation, "multiplicity", usage_id))
        .unwrap_or_else(|| "not found".to_string());
    let mut transcript = vec![
        "checking rotor part usage...".to_string(),
        "parsing source...".to_string(),
        "building assessment fact base...".to_string(),
        format!("running core assessment `{}`...", report.id),
        format!(
            "part usage `rotors`: {}",
            if rotor_usage_id.is_some() { "found" } else { "not found" }
        ),
        format!("type: {found_type}"),
        format!("multiplicity: {found_multiplicity}"),
        "expected multiplicity: 6".to_string(),
    ];
    if !diagnostics.is_empty() {
        transcript.push(format!("diagnostics: {}", diagnostics.len()));
    }
    for assertion in &report.assertions {
        transcript.push(format!(
            "assert {}: {}",
            assertion.id,
            match assertion.status {
                AssessmentStatus::Pass => "pass",
                AssessmentStatus::Failed => "failed",
            }
        ));
    }
    transcript.push(format!("result: {}", if passed { "pass" } else { "failed" }));

    Ok(success(
        json!({
            "assessmentId": request.assessment_id,
            "status": if passed { "pass" } else { "failed" },
            "command": command,
            "transcript": transcript,
            "facts": {
                "partUsageFound": rotor_usage_id.is_some(),
                "typeFound": found_type == "RotorAssembly",
                "type": if found_type == "not found" { None::<String> } else { Some(found_type) },
                "multiplicity": if found_multiplicity == "not found" { None::<String> } else { Some(found_multiplicity) },
                "expectedMultiplicity": "6",
            },
            "diagnostics": diagnostics,
        }),
        [("runtime", json!("wasm"))],
    ))
}

fn run_command_link_training_assessment(
    input: &str,
    request: TrainingAssessmentRequest,
) -> Result<Response, WasmError> {
    let expected_source = "controller::commandOut";
    let expected_target = "rotor::commandIn";
    let command = format!(
        "mercurio assess {} --check interface-connection --expect {expected_source}->{expected_target}",
        request.filename
    );
    let parse_report = match parse_sysml_recovering(input) {
        Ok(report) => report,
        Err(diagnostic) => {
            return Ok(success(
                json!({
                    "assessmentId": request.assessment_id,
                    "status": "failed",
                    "command": command,
                    "transcript": [
                        "checking interface connection...",
                        "parsing source...",
                        format!("parse error: {}", diagnostic.message),
                        "result: failed",
                    ],
                    "facts": {
                        "connectionFound": false,
                        "source": null,
                        "target": null,
                        "expectedSource": expected_source,
                        "expectedTarget": expected_target,
                    },
                    "diagnostics": [snippet_diagnostic(&diagnostic)],
                }),
                [("runtime", json!("wasm"))],
            ));
        }
    };

    let diagnostics = parse_report
        .diagnostics
        .iter()
        .map(snippet_diagnostic)
        .collect::<Vec<_>>();
    let facts = sysml_module_assessment_facts(&parse_report.module);
    let evaluation = evaluate(facts, &[])?;
    let spec = command_link_assessment_spec();
    let report = run_evaluation_assessment(&evaluation, &spec)?;
    let passed = diagnostics.is_empty() && report.status == AssessmentStatus::Pass;
    let interface_id = first_interface_usage_id(&evaluation);
    let connection_id = interface_usage_with_endpoints(&evaluation, expected_source, expected_target);
    let found_source = connection_id
        .as_deref()
        .and_then(|id| fact_value(&evaluation, "connected_source", id))
        .unwrap_or_else(|| "not found".to_string());
    let found_target = connection_id
        .as_deref()
        .and_then(|id| fact_value(&evaluation, "connected_target", id))
        .unwrap_or_else(|| "not found".to_string());
    let mut transcript = vec![
        "checking interface connection...".to_string(),
        "parsing source...".to_string(),
        "building assessment fact base...".to_string(),
        format!("running core assessment `{}`...", report.id),
        format!(
            "interface usage: {}",
            interface_id.as_deref().unwrap_or("not found")
        ),
        format!("interface connection: {}", connection_id.as_deref().unwrap_or("not found")),
        format!("source endpoint: {found_source}"),
        format!("target endpoint: {found_target}"),
        format!("expected: {expected_source} -> {expected_target}"),
    ];
    if !diagnostics.is_empty() {
        transcript.push(format!("diagnostics: {}", diagnostics.len()));
    }
    for assertion in &report.assertions {
        transcript.push(format!(
            "assert {}: {}",
            assertion.id,
            match assertion.status {
                AssessmentStatus::Pass => "pass",
                AssessmentStatus::Failed => "failed",
            }
        ));
    }
    transcript.push(format!("result: {}", if passed { "pass" } else { "failed" }));

    Ok(success(
        json!({
            "assessmentId": request.assessment_id,
            "status": if passed { "pass" } else { "failed" },
            "command": command,
            "transcript": transcript,
            "facts": {
                "interfaceFound": interface_id.is_some(),
                "connectionFound": connection_id.is_some(),
                "interface": interface_id,
                "connection": connection_id,
                "source": if found_source == "not found" { None::<String> } else { Some(found_source) },
                "target": if found_target == "not found" { None::<String> } else { Some(found_target) },
                "expectedSource": expected_source,
                "expectedTarget": expected_target,
            },
            "diagnostics": diagnostics,
        }),
        [("runtime", json!("wasm"))],
    ))
}

fn package_structure_assessment_spec(expected_package_count: usize) -> AssessmentSpec {
    AssessmentSpec {
        id: "training.packages.1_1".to_string(),
        title: "Package structure".to_string(),
        assertions: vec![
            AssessmentAssertion {
                id: "two-top-level-packages".to_string(),
                description: "Model declares the expected number of top-level packages".to_string(),
                query: AssessmentQuery {
                    find: vec!["P".to_string()],
                    where_atoms: vec![Atom {
                        predicate: "top_level_package".to_string(),
                        terms: vec![Term::Var("P".to_string())],
                    }],
                },
                expect: AssessmentExpectation::CountEq {
                    value: expected_package_count,
                },
            },
            AssessmentAssertion {
                id: "has-uav-library".to_string(),
                description: "One package is named UavLibrary".to_string(),
                query: AssessmentQuery {
                    find: vec!["P".to_string()],
                    where_atoms: vec![
                        Atom {
                            predicate: "package".to_string(),
                            terms: vec![Term::Var("P".to_string())],
                        },
                        Atom {
                            predicate: "name".to_string(),
                            terms: vec![
                                Term::Var("P".to_string()),
                                Term::Const("UavLibrary".to_string()),
                            ],
                        },
                    ],
                },
                expect: AssessmentExpectation::Exists,
            },
            AssessmentAssertion {
                id: "has-uav-system".to_string(),
                description: "One package is named UavSystem".to_string(),
                query: AssessmentQuery {
                    find: vec!["P".to_string()],
                    where_atoms: vec![
                        Atom {
                            predicate: "package".to_string(),
                            terms: vec![Term::Var("P".to_string())],
                        },
                        Atom {
                            predicate: "name".to_string(),
                            terms: vec![
                                Term::Var("P".to_string()),
                                Term::Const("UavSystem".to_string()),
                            ],
                        },
                    ],
                },
                expect: AssessmentExpectation::Exists,
            },
        ],
    }
}

fn rotors_multiplicity_assessment_spec() -> AssessmentSpec {
    AssessmentSpec {
        id: "training.parts.1_2".to_string(),
        title: "Rotor multiplicity".to_string(),
        assertions: vec![
            named_part_usage_assertion("has-rotors-part-usage", "Model declares a part usage named rotors"),
            AssessmentAssertion {
                id: "rotors-type".to_string(),
                description: "rotors is typed by RotorAssembly".to_string(),
                query: AssessmentQuery {
                    find: vec!["U".to_string()],
                    where_atoms: vec![
                        Atom {
                            predicate: "part_usage".to_string(),
                            terms: vec![Term::Var("U".to_string())],
                        },
                        Atom {
                            predicate: "name".to_string(),
                            terms: vec![
                                Term::Var("U".to_string()),
                                Term::Const("rotors".to_string()),
                            ],
                        },
                        Atom {
                            predicate: "type".to_string(),
                            terms: vec![
                                Term::Var("U".to_string()),
                                Term::Const("RotorAssembly".to_string()),
                            ],
                        },
                    ],
                },
                expect: AssessmentExpectation::Exists,
            },
            AssessmentAssertion {
                id: "rotors-multiplicity-six".to_string(),
                description: "rotors has multiplicity 6".to_string(),
                query: AssessmentQuery {
                    find: vec!["U".to_string()],
                    where_atoms: vec![
                        Atom {
                            predicate: "part_usage".to_string(),
                            terms: vec![Term::Var("U".to_string())],
                        },
                        Atom {
                            predicate: "name".to_string(),
                            terms: vec![
                                Term::Var("U".to_string()),
                                Term::Const("rotors".to_string()),
                            ],
                        },
                        Atom {
                            predicate: "multiplicity".to_string(),
                            terms: vec![
                                Term::Var("U".to_string()),
                                Term::Const("6".to_string()),
                            ],
                        },
                    ],
                },
                expect: AssessmentExpectation::Exists,
            },
        ],
    }
}

fn command_link_assessment_spec() -> AssessmentSpec {
    AssessmentSpec {
        id: "training.ports.1_3".to_string(),
        title: "Command interface connection".to_string(),
        assertions: vec![
            AssessmentAssertion {
                id: "has-interface-usage".to_string(),
                description: "Model declares an interface usage".to_string(),
                query: AssessmentQuery {
                    find: vec!["I".to_string()],
                    where_atoms: vec![Atom {
                        predicate: "interface_usage".to_string(),
                        terms: vec![Term::Var("I".to_string())],
                    }],
                },
                expect: AssessmentExpectation::Exists,
            },
            AssessmentAssertion {
                id: "controller-commandout-source".to_string(),
                description: "controller.commandOut is the source endpoint".to_string(),
                query: AssessmentQuery {
                    find: vec!["C".to_string()],
                    where_atoms: vec![
                        Atom {
                            predicate: "connected_source".to_string(),
                            terms: vec![
                                Term::Var("C".to_string()),
                                Term::Const("controller::commandOut".to_string()),
                            ],
                        },
                        Atom {
                            predicate: "connected_target".to_string(),
                            terms: vec![
                                Term::Var("C".to_string()),
                                Term::Const("rotor::commandIn".to_string()),
                            ],
                        },
                    ],
                },
                expect: AssessmentExpectation::Exists,
            },
            AssessmentAssertion {
                id: "rotor-commandin-target".to_string(),
                description: "rotor.commandIn is the target endpoint".to_string(),
                query: AssessmentQuery {
                    find: vec!["C".to_string()],
                    where_atoms: vec![
                        Atom {
                            predicate: "connected_source".to_string(),
                            terms: vec![
                                Term::Var("C".to_string()),
                                Term::Const("controller::commandOut".to_string()),
                            ],
                        },
                        Atom {
                            predicate: "connected_target".to_string(),
                            terms: vec![
                                Term::Var("C".to_string()),
                                Term::Const("rotor::commandIn".to_string()),
                            ],
                        },
                    ],
                },
                expect: AssessmentExpectation::Exists,
            },
        ],
    }
}

fn package_bindings(evaluation: &mercurio_core::Evaluation) -> Vec<String> {
    let mut packages = evaluation
        .facts()
        .iter()
        .filter_map(|fact| match (fact.predicate.as_str(), fact.terms.as_slice()) {
            ("top_level_package", [package]) => Some(package.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    packages.sort();
    packages
}

fn named_part_usage_assertion(id: &str, description: &str) -> AssessmentAssertion {
    AssessmentAssertion {
        id: id.to_string(),
        description: description.to_string(),
        query: AssessmentQuery {
            find: vec!["U".to_string()],
            where_atoms: vec![
                Atom {
                    predicate: "part_usage".to_string(),
                    terms: vec![Term::Var("U".to_string())],
                },
                Atom {
                    predicate: "name".to_string(),
                    terms: vec![
                        Term::Var("U".to_string()),
                        Term::Const("rotors".to_string()),
                    ],
                },
            ],
        },
        expect: AssessmentExpectation::Exists,
    }
}

fn named_part_usage_id(evaluation: &mercurio_core::Evaluation, name: &str) -> Option<String> {
    evaluation
        .facts()
        .iter()
        .filter_map(|fact| match (fact.predicate.as_str(), fact.terms.as_slice()) {
            ("name", [usage_id, usage_name]) if usage_name == name => Some(usage_id.clone()),
            _ => None,
        })
        .find(|usage_id| evaluation.contains("part_usage", &[usage_id]))
}

fn first_interface_usage_id(evaluation: &mercurio_core::Evaluation) -> Option<String> {
    evaluation
        .facts()
        .iter()
        .find_map(|fact| match (fact.predicate.as_str(), fact.terms.as_slice()) {
            ("interface_usage", [usage_id]) => Some(usage_id.clone()),
            _ => None,
        })
}

fn interface_usage_with_endpoints(
    evaluation: &mercurio_core::Evaluation,
    source: &str,
    target: &str,
) -> Option<String> {
    evaluation
        .facts()
        .iter()
        .filter_map(|fact| match (fact.predicate.as_str(), fact.terms.as_slice()) {
            ("interface_usage", [usage_id]) => Some(usage_id.clone()),
            _ => None,
        })
        .find(|usage_id| {
            evaluation.contains("connected_source", &[usage_id, source])
                && evaluation.contains("connected_target", &[usage_id, target])
        })
}

fn fact_value(
    evaluation: &mercurio_core::Evaluation,
    predicate: &str,
    subject: &str,
) -> Option<String> {
    evaluation
        .facts()
        .iter()
        .find_map(|fact| match (fact.predicate.as_str(), fact.terms.as_slice()) {
            (actual_predicate, [actual_subject, value])
                if actual_predicate == predicate && actual_subject == subject =>
            {
                Some(value.clone())
            }
            _ => None,
        })
}

fn snippet_diagnostic(diagnostic: &mercurio_core::frontend::diagnostics::Diagnostic) -> Value {
    let (line, column) = diagnostic
        .span
        .as_ref()
        .map(|span| (span.start_line, span.start_col))
        .unwrap_or((1, 1));
    json!({
        "severity": "error",
        "message": diagnostic.message,
        "startLineNumber": line,
        "startColumn": column,
        "endLineNumber": diagnostic.span.as_ref().map(|span| span.end_line).unwrap_or(line),
        "endColumn": diagnostic.span.as_ref().map(|span| span.end_col).unwrap_or(column),
        "start_line_number": line,
        "start_column": column,
        "end_line_number": diagnostic.span.as_ref().map(|span| span.end_line).unwrap_or(line),
        "end_column": diagnostic.span.as_ref().map(|span| span.end_col).unwrap_or(column),
        "line": line,
        "column": column,
    })
}

fn push_ast_symbol(symbols: &mut Vec<Value>, id: &str, kind: &str, label: &str, span: &SourceSpan) {
    symbols.push(json!({
        "id": id,
        "kind": kind,
        "label": label,
        "startLineNumber": span.start_line,
        "start_line_number": span.start_line,
    }));
}

fn declaration_outline_node(
    declaration: &Declaration,
    owner: Option<&str>,
    symbols: &mut Vec<Value>,
) -> Value {
    match declaration {
        Declaration::Package(package) => {
            let name = package.name.as_colon_string();
            let id = scoped_ast_id(owner, &name);
            package_outline_node(&id, &name, &package.span, &package.members, symbols)
        }
        Declaration::PartDefinition(definition) => {
            let id = scoped_ast_id(owner, &definition.name);
            push_ast_symbol(symbols, &id, "PartDefinition", &definition.name, &definition.span);
            let mut children = definition
                .members
                .iter()
                .map(|member| declaration_outline_node(member, Some(&id), symbols))
                .collect::<Vec<_>>();
            children.extend(definition.part_members.iter().map(|member| {
                part_usage_outline_node(member, Some(&id), symbols)
            }));
            json!({
                "id": id,
                "elementId": id,
                "element_id": id,
                "label": definition.name,
                "kind": "PartDefinition",
                "properties": ast_properties(&definition.name, &definition.span),
                "children": children,
            })
        }
        Declaration::PartUsage(usage) => part_usage_outline_node(usage, owner, symbols),
        Declaration::GenericDefinition(definition) => {
            let id = scoped_ast_id(owner, &definition.name);
            let kind = format!("{}Definition", pascal_keyword(&definition.keyword));
            push_ast_symbol(symbols, &id, &kind, &definition.name, &definition.span);
            let children = definition
                .members
                .iter()
                .map(|member| declaration_outline_node(member, Some(&id), symbols))
                .collect::<Vec<_>>();
            json!({
                "id": id,
                "elementId": id,
                "element_id": id,
                "label": definition.name,
                "kind": kind,
                "properties": ast_properties(&definition.name, &definition.span),
                "children": children,
            })
        }
        Declaration::GenericUsage(usage) => {
            let id = scoped_ast_id(owner, &usage.name);
            let kind = format!("{}Usage", pascal_keyword(&usage.keyword));
            push_ast_symbol(symbols, &id, &kind, &usage.name, &usage.span);
            let children = usage
                .body_members
                .iter()
                .map(|member| declaration_outline_node(member, Some(&id), symbols))
                .collect::<Vec<_>>();
            json!({
                "id": id,
                "elementId": id,
                "element_id": id,
                "label": usage.name,
                "kind": kind,
                "properties": ast_properties(&usage.name, &usage.span),
                "children": children,
            })
        }
        Declaration::Import(import) => {
            let name = import.path.as_colon_string();
            let id = scoped_ast_id(owner, &name);
            push_ast_symbol(symbols, &id, "Import", &name, &import.span);
            json!({
                "id": id,
                "elementId": id,
                "element_id": id,
                "label": name,
                "kind": "Import",
                "properties": ast_properties(&name, &import.span),
                "children": [],
            })
        }
        Declaration::Alias(alias) => {
            let id = scoped_ast_id(owner, &alias.name);
            push_ast_symbol(symbols, &id, "Alias", &alias.name, &alias.span);
            json!({
                "id": id,
                "elementId": id,
                "element_id": id,
                "label": alias.name,
                "kind": "Alias",
                "properties": ast_properties(&alias.name, &alias.span),
                "children": [],
            })
        }
    }
}

fn package_outline_node(
    id: &str,
    name: &str,
    span: &SourceSpan,
    members: &[Declaration],
    symbols: &mut Vec<Value>,
) -> Value {
    push_ast_symbol(symbols, id, "Package", name, span);
    let children = members
        .iter()
        .map(|member| declaration_outline_node(member, Some(id), symbols))
        .collect::<Vec<_>>();
    json!({
        "id": id,
        "elementId": id,
        "element_id": id,
        "label": name,
        "kind": "Package",
        "properties": ast_properties(name, span),
        "children": children,
    })
}

fn part_usage_outline_node(
    usage: &PartUsageDecl,
    owner: Option<&str>,
    symbols: &mut Vec<Value>,
) -> Value {
    let id = scoped_ast_id(owner, &usage.name);
    push_ast_symbol(symbols, &id, "PartUsage", &usage.name, &usage.span);
    let children = usage
        .body_members
        .iter()
        .map(|member| declaration_outline_node(member, Some(&id), symbols))
        .collect::<Vec<_>>();
    json!({
        "id": id,
        "elementId": id,
        "element_id": id,
        "label": usage.name,
        "kind": "PartUsage",
        "properties": ast_properties(&usage.name, &usage.span),
        "children": children,
    })
}

fn scoped_ast_id(owner: Option<&str>, name: &str) -> String {
    owner
        .map(|owner| format!("{owner}::{name}"))
        .unwrap_or_else(|| name.to_string())
}

fn ast_properties(name: &str, span: &SourceSpan) -> Value {
    json!({
        "name": name,
        "metadata": {
            "name": name,
            "source_span": {
                "start_line": span.start_line,
                "start_col": span.start_col,
                "end_line": span.end_line,
                "end_col": span.end_col,
            },
        },
        "source_span": {
            "start_line": span.start_line,
            "start_col": span.start_col,
            "end_line": span.end_line,
            "end_col": span.end_col,
        },
    })
}

fn pascal_keyword(keyword: &str) -> String {
    keyword
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<String>()
}

fn default_source_name() -> String {
    "memory.sysml".to_string()
}

fn run_runtime_query(runtime: &Runtime, query: RuntimeQuery) -> Result<Value, WasmError> {
    match query.kind {
        RuntimeQueryKind::Subtypes => {
            let type_id = required(query.type_id, "typeId")?;
            let result = runtime.get_subtypes(&type_id)?;
            Ok(json!({
                "value": result.value,
                "explanation": result.explanation,
            }))
        }
        RuntimeQueryKind::Features => {
            let type_id = required(query.type_id, "typeId")?;
            let result = runtime.get_features(&type_id)?;
            Ok(json!({
                "value": result.value,
                "explanation": result.explanation,
            }))
        }
        RuntimeQueryKind::Evaluate => {
            let feature_id = required(query.feature_id, "featureId")?;
            let owner_id = required(query.owner_id, "ownerId")?;
            let context = execution_context(query.context);
            let result = runtime.evaluate(&feature_id, &owner_id, &context)?;
            Ok(json!({
                "value": result.value,
                "explanation": result.explanation,
            }))
        }
    }
}

fn execution_context(context: RuntimeContextDto) -> ExecutionContext {
    let values = context
        .values
        .into_iter()
        .map(|entry| ((entry.owner_id, entry.feature_id), entry.value))
        .collect();
    ExecutionContext {
        values,
        version: context.version,
    }
}

fn required(value: Option<String>, field: &str) -> Result<String, WasmError> {
    value.ok_or_else(|| WasmError::new("query", format!("missing runtime query field: {field}")))
}

fn from_js<T>(value: JsValue) -> Result<T, WasmError>
where
    T: serde::de::DeserializeOwned,
{
    serde_wasm_bindgen::from_value(value).map_err(|err| WasmError::new("js", err.to_string()))
}

fn to_js<T>(value: &T) -> JsValue
where
    T: Serialize,
{
    serde_wasm_bindgen::to_value(value).unwrap_or_else(|err| {
        JsValue::from_str(&format!("failed to serialize wasm response: {err}"))
    })
}

fn json_response(action: impl FnOnce() -> Result<Response, WasmError>) -> JsValue {
    match action() {
        Ok(response) => to_js(&response),
        Err(error) => to_js(&Response {
            ok: false,
            value: None,
            diagnostics: json!([]),
            errors: vec![error],
            metadata: BTreeMap::new(),
        }),
    }
}

fn success<const N: usize>(value: Value, metadata_items: [(&str, Value); N]) -> Response {
    Response {
        ok: true,
        value: Some(value),
        diagnostics: json!([]),
        errors: Vec::new(),
        metadata: metadata(metadata_items),
    }
}

fn error_response(code: &str, message: String, diagnostics: Option<Value>) -> Response {
    Response {
        ok: false,
        value: None,
        diagnostics: diagnostics.unwrap_or_else(|| json!([])),
        errors: vec![WasmError::new(code, message)],
        metadata: BTreeMap::new(),
    }
}

fn metadata<const N: usize>(items: [(&str, Value); N]) -> BTreeMap<String, Value> {
    items
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

fn js_error(error: WasmError) -> JsValue {
    JsValue::from_str(&error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_stdlib_is_valid_kir() {
        let document = load_stdlib(None).unwrap();
        assert!(!document.elements.is_empty());
    }

    #[test]
    fn default_stdlib_resolves_port_and_interface_definitions() {
        let stdlib = load_stdlib(None).unwrap();
        let report = compile_sysml_text_with_context_report(
            "package Demo {
                item def Command;

                port def CommandPort {
                    item command: Command;
                }

                interface def CommandInterface {
                    end controller: CommandPort;
                    end rotor: CommandPort;
                }
            }",
            "ports.sysml",
            &[],
            &stdlib,
        );

        assert!(
            report.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            report.diagnostics
        );
        assert!(report.document.is_some());
    }

    #[test]
    fn command_link_training_assessment_passes_for_expected_endpoints() {
        let response = run_command_link_training_assessment(
            "package Demo {
                item def Command;
                port def CommandPort { item command: Command; }
                interface def CommandInterface {
                    end controller: CommandPort;
                    end rotor: CommandPort;
                }
                part def FlightComputer { port commandOut: CommandPort; }
                part def RotorAssembly { port commandIn: CommandPort; }
                part def UavSystem {
                    part controller: FlightComputer;
                    part rotor: RotorAssembly;
                    interface motorCommandLink: CommandInterface
                        connect controller.commandOut to rotor.commandIn;
                }
            }",
            TrainingAssessmentRequest {
                assessment_id: "interface-connection.command-link".to_string(),
                filename: "ports.sysml".to_string(),
                expected_package_count: None,
            },
        )
        .unwrap();

        assert_eq!(response.value.unwrap()["status"], "pass");
    }

    #[test]
    fn session_merges_user_sources_with_stdlib() {
        let stdlib = load_stdlib(None).unwrap();
        let module = parse_sysml_recovering("package Demo { }").unwrap().module;
        let document =
            compile_sysml_text_with_context_report("package Demo { }", "demo.sysml", &[], &stdlib)
                .document
                .unwrap();
        let mut session = MercurioSession {
            stdlib,
            sources: Vec::new(),
        };

        session.sources.push(SessionSource {
            source_name: "demo.sysml".to_string(),
            language: SourceLanguage::Sysml,
            module,
            document,
        });
        assert!(session.merged_document().unwrap().elements.len() > session.stdlib.elements.len());
    }
}
