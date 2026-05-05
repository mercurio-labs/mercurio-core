use std::time::Duration;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const DEFAULT_OPENAI_MODEL: &str = "gpt-5.4-mini";
const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1/responses";
const DEFAULT_AZURE_OPENAI_PATH: &str = "/openai/v1/responses";
const DEFAULT_HTTP_TIMEOUT_SECS: u64 = 20;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningProviderKind {
    Heuristic,
    OpenAi,
    AzureOpenAi,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReasoningProviderStatus {
    pub kind: ReasoningProviderKind,
    pub provider_label: String,
    pub detail: String,
    pub structured_outputs: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticChangeKind {
    Added,
    Removed,
    Changed,
    Unchanged,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticChangeItem {
    pub kind: SemanticChangeKind,
    pub element_id: String,
    pub element_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_properties: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_relationships: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticSummaryRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_files: Vec<String>,
    #[serde(default)]
    pub changes: Vec<SemanticChangeItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticSummaryResponse {
    pub title: String,
    pub body: Vec<String>,
    pub provider: ReasoningProviderStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatMessageRole {
    Developer,
    Assistant,
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: ChatMessageRole,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatCompletionRequest {
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatCompletionResponse {
    pub message: String,
    pub provider: ReasoningProviderStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AskMercurioTask {
    DesignQuestion,
    DiagramRequest,
    ViewRequest,
    #[serde(rename = "proposal_draft", alias = "pr_draft")]
    PrDraft,
    General,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AskMercurioProjectContext {
    pub project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagram_root_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagram_root_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AskMercurioRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_hint: Option<AskMercurioTask>,
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AskMercurioResponse {
    pub message: String,
    pub task: AskMercurioTask,
    pub provider: ReasoningProviderStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<AskMercurioProjectContext>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<AskMercurioCitation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<AskMercurioArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AskMercurioCitation {
    pub label: String,
    pub target_type: String,
    pub target_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum AskMercurioArtifact {
    DiagramSpec(Value),
    RequirementsView(Value),
    ProposalDraft(ProposalDraft),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProposalDraft {
    pub title: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_base_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_head_branch: Option<String>,
    pub checklist: Vec<String>,
    #[serde(default)]
    pub linked_semantic_elements: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStructuredResponse {
    output: Vec<OpenAiOutputItem>,
}

#[derive(Debug, Deserialize)]
struct OpenAiOutputItem {
    #[serde(default)]
    content: Vec<OpenAiContentItem>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum OpenAiContentItem {
    #[serde(rename = "output_text")]
    OutputText { text: String },
    #[serde(rename = "refusal")]
    Refusal { refusal: String },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct SemanticSummaryEnvelope {
    title: String,
    body: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ConnectionProbeEnvelope {
    ok: bool,
}

#[derive(Debug, Clone)]
pub enum ResolvedReasoningProvider {
    Heuristic(HeuristicReasoningProvider),
    OpenAi(OpenAiReasoningProvider),
    AzureOpenAi(AzureOpenAiReasoningProvider),
}

#[derive(Debug, Clone)]
pub struct HeuristicReasoningProvider {
    status: ReasoningProviderStatus,
}

#[derive(Debug, Clone)]
pub struct OpenAiReasoningProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
    status: ReasoningProviderStatus,
    fallback: HeuristicReasoningProvider,
}

#[derive(Debug, Clone)]
pub struct AzureOpenAiReasoningProvider {
    client: Client,
    api_key: String,
    deployment: String,
    base_url: String,
    status: ReasoningProviderStatus,
    fallback: HeuristicReasoningProvider,
}

pub trait ReasoningProvider {
    fn provider_status(&self) -> ReasoningProviderStatus;

    fn test_connection(&self) -> Result<ReasoningProviderStatus, String>;

    fn summarize_semantic_changes(
        &self,
        request: &SemanticSummaryRequest,
    ) -> SemanticSummaryResponse;

    fn complete_chat(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, String>;
}

#[derive(Debug, Clone, Default)]
pub struct ReasoningProviderSecretOverrides {
    pub openai_api_key: Option<String>,
    pub azure_openai_api_key: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ReasoningProviderConfigOverrides {
    pub provider: Option<ReasoningProviderKind>,
    pub openai_model: Option<String>,
    pub openai_base_url: Option<String>,
    pub azure_openai_deployment: Option<String>,
    pub azure_openai_base_url: Option<String>,
}

pub fn default_reasoning_provider() -> ResolvedReasoningProvider {
    resolve_reasoning_provider_from_env(&ReasoningProviderSecretOverrides::default())
}

pub fn default_reasoning_provider_with_secret_overrides(
    secrets: ReasoningProviderSecretOverrides,
) -> ResolvedReasoningProvider {
    resolve_reasoning_provider_from_env(&secrets)
}

pub fn default_reasoning_provider_status() -> ReasoningProviderStatus {
    default_reasoning_provider().provider_status()
}

pub fn default_reasoning_provider_status_with_secret_overrides(
    secrets: ReasoningProviderSecretOverrides,
) -> ReasoningProviderStatus {
    default_reasoning_provider_with_secret_overrides(secrets).provider_status()
}

pub fn test_default_reasoning_provider_connection() -> Result<ReasoningProviderStatus, String> {
    default_reasoning_provider().test_connection()
}

pub fn test_default_reasoning_provider_connection_with_secret_overrides(
    secrets: ReasoningProviderSecretOverrides,
) -> Result<ReasoningProviderStatus, String> {
    default_reasoning_provider_with_secret_overrides(secrets).test_connection()
}

pub fn configured_reasoning_provider(
    config: ReasoningProviderConfigOverrides,
    secrets: ReasoningProviderSecretOverrides,
) -> ResolvedReasoningProvider {
    match config.provider {
        Some(ReasoningProviderKind::AzureOpenAi) => {
            azure_openai_provider_from_config(&config, &secrets)
                .map(ResolvedReasoningProvider::AzureOpenAi)
                .unwrap_or_else(|| ResolvedReasoningProvider::Heuristic(heuristic_provider()))
        }
        Some(ReasoningProviderKind::OpenAi) => openai_provider_from_config(&config, &secrets)
            .map(ResolvedReasoningProvider::OpenAi)
            .unwrap_or_else(|| ResolvedReasoningProvider::Heuristic(heuristic_provider())),
        _ => default_reasoning_provider_with_secret_overrides(secrets),
    }
}

pub fn test_configured_reasoning_provider_connection(
    config: ReasoningProviderConfigOverrides,
    secrets: ReasoningProviderSecretOverrides,
) -> Result<ReasoningProviderStatus, String> {
    match config.provider {
        Some(ReasoningProviderKind::AzureOpenAi) => {
            let provider =
                azure_openai_provider_from_config(&config, &secrets).ok_or_else(|| {
                    let mut missing = Vec::new();
                    if config
                        .azure_openai_deployment
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .is_none()
                    {
                        missing.push("deployment");
                    }
                    if config
                        .azure_openai_base_url
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .is_none()
                    {
                        missing.push("base URL");
                    }
                    if secrets
                        .azure_openai_api_key
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .is_none()
                    {
                        missing.push("stored API key");
                    }
                    format!(
                        "Azure OpenAI settings are incomplete. Missing {}.",
                        missing.join(", ")
                    )
                })?;
            provider.test_connection()
        }
        Some(ReasoningProviderKind::OpenAi) => {
            let provider = openai_provider_from_config(&config, &secrets).ok_or_else(|| {
                let mut missing = Vec::new();
                if config
                    .openai_model
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_none()
                {
                    missing.push("model");
                }
                if config
                    .openai_base_url
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_none()
                {
                    missing.push("base URL");
                }
                if secrets
                    .openai_api_key
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_none()
                {
                    missing.push("stored API key");
                }
                format!(
                    "OpenAI settings are incomplete. Missing {}.",
                    missing.join(", ")
                )
            })?;
            provider.test_connection()
        }
        _ => default_reasoning_provider_with_secret_overrides(secrets).test_connection(),
    }
}

pub fn summarize_semantic_changes(request: &SemanticSummaryRequest) -> SemanticSummaryResponse {
    default_reasoning_provider().summarize_semantic_changes(request)
}

pub fn summarize_semantic_changes_with_secret_overrides(
    request: &SemanticSummaryRequest,
    secrets: ReasoningProviderSecretOverrides,
) -> SemanticSummaryResponse {
    default_reasoning_provider_with_secret_overrides(secrets).summarize_semantic_changes(request)
}

pub fn complete_chat_with_secret_overrides(
    request: &ChatCompletionRequest,
    secrets: ReasoningProviderSecretOverrides,
) -> Result<ChatCompletionResponse, String> {
    default_reasoning_provider_with_secret_overrides(secrets).complete_chat(request)
}

pub fn ask_mercurio(
    request: &AskMercurioRequest,
    project: Option<AskMercurioProjectContext>,
    context: Vec<String>,
) -> Result<AskMercurioResponse, String> {
    ask_mercurio_with_provider(default_reasoning_provider(), request, project, context)
}

pub fn ask_mercurio_with_config(
    config: ReasoningProviderConfigOverrides,
    secrets: ReasoningProviderSecretOverrides,
    request: &AskMercurioRequest,
    project: Option<AskMercurioProjectContext>,
    context: Vec<String>,
) -> Result<AskMercurioResponse, String> {
    ask_mercurio_with_provider(
        configured_reasoning_provider(config, secrets),
        request,
        project,
        context,
    )
}

fn ask_mercurio_with_provider(
    provider: ResolvedReasoningProvider,
    request: &AskMercurioRequest,
    project: Option<AskMercurioProjectContext>,
    context: Vec<String>,
) -> Result<AskMercurioResponse, String> {
    let task = request
        .task_hint
        .clone()
        .unwrap_or_else(|| classify_ask_mercurio_task(latest_user_content(&request.messages)));
    let mut chat_context = vec![ask_mercurio_developer_context(&task)];
    chat_context.extend(context);
    let chat_request = ChatCompletionRequest {
        messages: request.messages.clone(),
        context: chat_context,
    };
    let chat = provider
        .complete_chat(&chat_request)
        .unwrap_or_else(|_| heuristic_provider().complete_chat(&chat_request).unwrap());
    let citations =
        ask_mercurio_citations(project.as_ref(), latest_user_content(&request.messages));
    let artifacts = ask_mercurio_artifacts(
        &task,
        project.as_ref(),
        latest_user_content(&request.messages),
    );

    Ok(AskMercurioResponse {
        message: chat.message,
        task,
        provider: chat.provider,
        project,
        citations,
        artifacts,
    })
}

pub fn complete_configured_chat(
    config: ReasoningProviderConfigOverrides,
    secrets: ReasoningProviderSecretOverrides,
    request: &ChatCompletionRequest,
) -> Result<ChatCompletionResponse, String> {
    match config.provider {
        Some(ReasoningProviderKind::AzureOpenAi) => {
            let provider =
                azure_openai_provider_from_config(&config, &secrets).ok_or_else(|| {
                    configured_provider_missing_message(
                        &config,
                        &secrets,
                        ReasoningProviderKind::AzureOpenAi,
                    )
                })?;
            provider.complete_chat(request)
        }
        Some(ReasoningProviderKind::OpenAi) => {
            let provider = openai_provider_from_config(&config, &secrets).ok_or_else(|| {
                configured_provider_missing_message(
                    &config,
                    &secrets,
                    ReasoningProviderKind::OpenAi,
                )
            })?;
            provider.complete_chat(request)
        }
        _ => complete_chat_with_secret_overrides(request, secrets),
    }
}

impl ReasoningProvider for ResolvedReasoningProvider {
    fn provider_status(&self) -> ReasoningProviderStatus {
        match self {
            Self::Heuristic(provider) => provider.provider_status(),
            Self::OpenAi(provider) => provider.provider_status(),
            Self::AzureOpenAi(provider) => provider.provider_status(),
        }
    }

    fn test_connection(&self) -> Result<ReasoningProviderStatus, String> {
        match self {
            Self::Heuristic(provider) => provider.test_connection(),
            Self::OpenAi(provider) => provider.test_connection(),
            Self::AzureOpenAi(provider) => provider.test_connection(),
        }
    }

    fn summarize_semantic_changes(
        &self,
        request: &SemanticSummaryRequest,
    ) -> SemanticSummaryResponse {
        match self {
            Self::Heuristic(provider) => provider.summarize_semantic_changes(request),
            Self::OpenAi(provider) => provider.summarize_semantic_changes(request),
            Self::AzureOpenAi(provider) => provider.summarize_semantic_changes(request),
        }
    }

    fn complete_chat(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, String> {
        match self {
            Self::Heuristic(provider) => provider.complete_chat(request),
            Self::OpenAi(provider) => provider.complete_chat(request),
            Self::AzureOpenAi(provider) => provider.complete_chat(request),
        }
    }
}

impl ReasoningProvider for HeuristicReasoningProvider {
    fn provider_status(&self) -> ReasoningProviderStatus {
        self.status.clone()
    }

    fn test_connection(&self) -> Result<ReasoningProviderStatus, String> {
        Ok(self.status.clone())
    }

    fn summarize_semantic_changes(
        &self,
        request: &SemanticSummaryRequest,
    ) -> SemanticSummaryResponse {
        heuristic_semantic_summary(request, self.status.clone())
    }

    fn complete_chat(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, String> {
        Ok(heuristic_chat_completion(request, self.status.clone()))
    }
}

impl ReasoningProvider for OpenAiReasoningProvider {
    fn provider_status(&self) -> ReasoningProviderStatus {
        self.status.clone()
    }

    fn test_connection(&self) -> Result<ReasoningProviderStatus, String> {
        let payload = self.request_structured_json(
            "connection_probe",
            connection_probe_schema(),
            "Return JSON only. Respond with {\"ok\":true}.",
            "Confirm that the configured reasoning provider is reachable.",
        )?;
        let envelope: ConnectionProbeEnvelope =
            serde_json::from_value(payload).map_err(|error| error.to_string())?;
        if envelope.ok {
            Ok(self.status.clone())
        } else {
            Err("OpenAI provider returned an invalid connection probe response.".to_string())
        }
    }

    fn summarize_semantic_changes(
        &self,
        request: &SemanticSummaryRequest,
    ) -> SemanticSummaryResponse {
        match self.summarize_via_openai(request) {
            Ok(response) => response,
            Err(_) => self.fallback.summarize_semantic_changes(request),
        }
    }

    fn complete_chat(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, String> {
        self.complete_chat_via_openai(request)
    }
}

impl ReasoningProvider for AzureOpenAiReasoningProvider {
    fn provider_status(&self) -> ReasoningProviderStatus {
        self.status.clone()
    }

    fn test_connection(&self) -> Result<ReasoningProviderStatus, String> {
        let payload = self.request_structured_json(
            "connection_probe",
            connection_probe_schema(),
            "Return JSON only. Respond with {\"ok\":true}.",
            "Confirm that the configured Azure OpenAI reasoning provider is reachable.",
        )?;
        let envelope: ConnectionProbeEnvelope =
            serde_json::from_value(payload).map_err(|error| error.to_string())?;
        if envelope.ok {
            Ok(self.status.clone())
        } else {
            Err("Azure OpenAI provider returned an invalid connection probe response.".to_string())
        }
    }

    fn summarize_semantic_changes(
        &self,
        request: &SemanticSummaryRequest,
    ) -> SemanticSummaryResponse {
        match self.summarize_via_azure(request) {
            Ok(response) => response,
            Err(_) => self.fallback.summarize_semantic_changes(request),
        }
    }

    fn complete_chat(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, String> {
        self.complete_chat_via_azure(request)
    }
}

impl OpenAiReasoningProvider {
    fn summarize_via_openai(
        &self,
        request: &SemanticSummaryRequest,
    ) -> Result<SemanticSummaryResponse, String> {
        let payload = self.request_structured_json(
            "semantic_change_summary",
            semantic_summary_schema(),
            semantic_summary_developer_prompt(),
            &semantic_summary_user_prompt(request),
        )?;
        let envelope: SemanticSummaryEnvelope =
            serde_json::from_value(payload).map_err(|error| error.to_string())?;
        Ok(SemanticSummaryResponse {
            title: envelope.title.trim().to_string(),
            body: envelope
                .body
                .into_iter()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty())
                .collect(),
            provider: self.status.clone(),
        })
    }

    fn request_structured_json(
        &self,
        schema_name: &str,
        schema: Value,
        developer_prompt: &str,
        user_prompt: &str,
    ) -> Result<Value, String> {
        request_openai_structured_json(
            &self.client,
            &self.base_url,
            &self.api_key,
            &self.model,
            schema_name,
            schema,
            developer_prompt,
            user_prompt,
        )
    }

    fn complete_chat_via_openai(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, String> {
        let message = request_openai_text(
            &self.client,
            &self.base_url,
            &self.api_key,
            &self.model,
            request,
        )?;
        Ok(ChatCompletionResponse {
            message,
            provider: self.status.clone(),
        })
    }
}

impl AzureOpenAiReasoningProvider {
    fn summarize_via_azure(
        &self,
        request: &SemanticSummaryRequest,
    ) -> Result<SemanticSummaryResponse, String> {
        let payload = self.request_structured_json(
            "semantic_change_summary",
            semantic_summary_schema(),
            semantic_summary_developer_prompt(),
            &semantic_summary_user_prompt(request),
        )?;
        let envelope: SemanticSummaryEnvelope =
            serde_json::from_value(payload).map_err(|error| error.to_string())?;
        Ok(SemanticSummaryResponse {
            title: envelope.title.trim().to_string(),
            body: envelope
                .body
                .into_iter()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty())
                .collect(),
            provider: self.status.clone(),
        })
    }

    fn request_structured_json(
        &self,
        schema_name: &str,
        schema: Value,
        developer_prompt: &str,
        user_prompt: &str,
    ) -> Result<Value, String> {
        request_openai_structured_json(
            &self.client,
            &self.base_url,
            &self.api_key,
            &self.deployment,
            schema_name,
            schema,
            developer_prompt,
            user_prompt,
        )
    }

    fn complete_chat_via_azure(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, String> {
        let message = request_openai_text(
            &self.client,
            &self.base_url,
            &self.api_key,
            &self.deployment,
            request,
        )?;
        Ok(ChatCompletionResponse {
            message,
            provider: self.status.clone(),
        })
    }
}

fn resolve_reasoning_provider_from_env(
    secrets: &ReasoningProviderSecretOverrides,
) -> ResolvedReasoningProvider {
    let requested = std::env::var("MERCURIO_AI_PROVIDER")
        .or_else(|_| std::env::var("MERCURIO_REASONING_PROVIDER"))
        .unwrap_or_default()
        .to_ascii_lowercase();

    if requested == "azure_openai" || requested == "azure-openai" {
        if let Some(provider) = azure_openai_provider_from_env(secrets) {
            return ResolvedReasoningProvider::AzureOpenAi(provider);
        }
    }

    if (requested == "openai" || requested.is_empty())
        && let Some(provider) = openai_provider_from_env(secrets)
    {
        return ResolvedReasoningProvider::OpenAi(provider);
    }

    ResolvedReasoningProvider::Heuristic(heuristic_provider())
}

fn heuristic_provider() -> HeuristicReasoningProvider {
    HeuristicReasoningProvider {
        status: ReasoningProviderStatus {
            kind: ReasoningProviderKind::Heuristic,
            provider_label: "Heuristic".to_string(),
            detail: "Local deterministic summaries; no external provider configured.".to_string(),
            structured_outputs: true,
            model_label: None,
        },
    }
}

fn openai_provider_from_env(
    secrets: &ReasoningProviderSecretOverrides,
) -> Option<OpenAiReasoningProvider> {
    let api_key = secrets
        .openai_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            std::env::var("OPENAI_API_KEY")
                .or_else(|_| std::env::var("MERCURIO_OPENAI_API_KEY"))
                .ok()
                .filter(|value| !value.trim().is_empty())
        })?;
    let model = std::env::var("MERCURIO_OPENAI_MODEL")
        .or_else(|_| std::env::var("OPENAI_MODEL"))
        .unwrap_or_else(|_| DEFAULT_OPENAI_MODEL.to_string());
    let base_url = std::env::var("MERCURIO_OPENAI_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_OPENAI_BASE_URL.to_string());

    Some(OpenAiReasoningProvider {
        client: http_client(),
        api_key,
        model: model.clone(),
        base_url,
        status: ReasoningProviderStatus {
            kind: ReasoningProviderKind::OpenAi,
            provider_label: "OpenAI".to_string(),
            detail: "OpenAI Responses API configured from environment.".to_string(),
            structured_outputs: true,
            model_label: Some(model),
        },
        fallback: heuristic_provider(),
    })
}

fn openai_provider_from_config(
    config: &ReasoningProviderConfigOverrides,
    secrets: &ReasoningProviderSecretOverrides,
) -> Option<OpenAiReasoningProvider> {
    let api_key = secrets
        .openai_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)?;
    let model = config
        .openai_model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_OPENAI_MODEL)
        .to_string();
    let base_url = config
        .openai_base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_OPENAI_BASE_URL)
        .to_string();

    Some(OpenAiReasoningProvider {
        client: http_client(),
        api_key,
        model: model.clone(),
        base_url,
        status: ReasoningProviderStatus {
            kind: ReasoningProviderKind::OpenAi,
            provider_label: "OpenAI".to_string(),
            detail:
                "OpenAI Responses API configured from application settings and stored credential."
                    .to_string(),
            structured_outputs: true,
            model_label: Some(model),
        },
        fallback: heuristic_provider(),
    })
}

fn azure_openai_provider_from_env(
    secrets: &ReasoningProviderSecretOverrides,
) -> Option<AzureOpenAiReasoningProvider> {
    let api_key = secrets
        .azure_openai_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            std::env::var("AZURE_OPENAI_API_KEY")
                .or_else(|_| std::env::var("MERCURIO_AZURE_OPENAI_API_KEY"))
                .ok()
                .filter(|value| !value.trim().is_empty())
        })?;
    let deployment = std::env::var("AZURE_OPENAI_DEPLOYMENT")
        .or_else(|_| std::env::var("MERCURIO_AZURE_OPENAI_DEPLOYMENT"))
        .ok()
        .filter(|value| !value.trim().is_empty())?;
    let base_url = std::env::var("MERCURIO_AZURE_OPENAI_BASE_URL")
        .or_else(|_| std::env::var("AZURE_OPENAI_BASE_URL"))
        .or_else(|_| std::env::var("AZURE_OPENAI_ENDPOINT"))
        .ok()
        .map(|value| normalize_azure_openai_base_url(&value))?;

    Some(AzureOpenAiReasoningProvider {
        client: http_client(),
        api_key,
        deployment: deployment.clone(),
        base_url,
        status: ReasoningProviderStatus {
            kind: ReasoningProviderKind::AzureOpenAi,
            provider_label: "Azure OpenAI".to_string(),
            detail: "Azure OpenAI Responses API configured from environment.".to_string(),
            structured_outputs: true,
            model_label: Some(deployment),
        },
        fallback: heuristic_provider(),
    })
}

fn azure_openai_provider_from_config(
    config: &ReasoningProviderConfigOverrides,
    secrets: &ReasoningProviderSecretOverrides,
) -> Option<AzureOpenAiReasoningProvider> {
    let api_key = secrets
        .azure_openai_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)?;
    let deployment = config
        .azure_openai_deployment
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let base_url = config
        .azure_openai_base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_azure_openai_base_url)?;

    Some(AzureOpenAiReasoningProvider {
        client: http_client(),
        api_key,
        deployment: deployment.clone(),
        base_url,
        status: ReasoningProviderStatus {
            kind: ReasoningProviderKind::AzureOpenAi,
            provider_label: "Azure OpenAI".to_string(),
            detail:
                "Azure OpenAI Responses API configured from application settings and stored credential."
                    .to_string(),
            structured_outputs: true,
            model_label: Some(deployment),
        },
        fallback: heuristic_provider(),
    })
}

fn http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
        .build()
        .unwrap_or_else(|_| Client::new())
}

fn request_openai_structured_json(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    schema_name: &str,
    schema: Value,
    developer_prompt: &str,
    user_prompt: &str,
) -> Result<Value, String> {
    let body = json!({
        "model": model,
        "input": [
            {
                "role": "developer",
                "content": developer_prompt,
            },
            {
                "role": "user",
                "content": user_prompt,
            }
        ],
        "text": {
            "format": {
                "type": "json_schema",
                "name": schema_name,
                "strict": true,
                "schema": schema,
            }
        }
    });

    let response = client
        .post(base_url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let body_text = response.text().map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!("AI provider request failed: {status} {body_text}"));
    }

    let envelope: OpenAiStructuredResponse =
        serde_json::from_str(&body_text).map_err(|error| error.to_string())?;
    let output_text = extract_output_text(&envelope)?;
    serde_json::from_str(&output_text).map_err(|error| error.to_string())
}

fn request_openai_text(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    request: &ChatCompletionRequest,
) -> Result<String, String> {
    let mut input = Vec::new();
    if !request.context.is_empty() {
        input.push(json!({
            "role": "developer",
            "content": format!("Use this Mercurio model context when it is relevant:\n{}", request.context.join("\n")),
        }));
    }
    input.extend(request.messages.iter().map(|message| {
        json!({
            "role": chat_role_name(&message.role),
            "content": message.content,
        })
    }));
    if input.is_empty() {
        return Err("Chat request must include at least one message.".to_string());
    }

    let body = json!({
        "model": model,
        "input": input,
    });

    let response = client
        .post(base_url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let body_text = response.text().map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!("AI provider request failed: {status} {body_text}"));
    }

    let envelope: OpenAiStructuredResponse =
        serde_json::from_str(&body_text).map_err(|error| error.to_string())?;
    extract_output_text(&envelope).map(|value| value.trim().to_string())
}

fn chat_role_name(role: &ChatMessageRole) -> &'static str {
    match role {
        ChatMessageRole::Developer => "developer",
        ChatMessageRole::Assistant => "assistant",
        ChatMessageRole::User => "user",
    }
}

fn configured_provider_missing_message(
    config: &ReasoningProviderConfigOverrides,
    secrets: &ReasoningProviderSecretOverrides,
    provider: ReasoningProviderKind,
) -> String {
    let mut missing = Vec::new();
    match provider {
        ReasoningProviderKind::AzureOpenAi => {
            if config
                .azure_openai_deployment
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
            {
                missing.push("deployment");
            }
            if config
                .azure_openai_base_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
            {
                missing.push("base URL");
            }
            if secrets
                .azure_openai_api_key
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
            {
                missing.push("stored API key");
            }
            format!(
                "Azure OpenAI settings are incomplete. Missing {}.",
                missing.join(", ")
            )
        }
        ReasoningProviderKind::OpenAi => {
            if config
                .openai_model
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
            {
                missing.push("model");
            }
            if config
                .openai_base_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
            {
                missing.push("base URL");
            }
            if secrets
                .openai_api_key
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
            {
                missing.push("stored API key");
            }
            format!(
                "OpenAI settings are incomplete. Missing {}.",
                missing.join(", ")
            )
        }
        ReasoningProviderKind::Heuristic => "No external AI provider is configured.".to_string(),
    }
}

fn heuristic_semantic_summary(
    request: &SemanticSummaryRequest,
    provider: ReasoningProviderStatus,
) -> SemanticSummaryResponse {
    let added = request
        .changes
        .iter()
        .filter(|change| change.kind == SemanticChangeKind::Added)
        .count();
    let removed = request
        .changes
        .iter()
        .filter(|change| change.kind == SemanticChangeKind::Removed)
        .count();
    let changed = request
        .changes
        .iter()
        .filter(|change| change.kind == SemanticChangeKind::Changed)
        .count();
    let title = request
        .title_hint
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| semantic_summary_title(added, changed, removed));

    let mut body = Vec::new();
    if !request.changed_files.is_empty() {
        body.push(format!(
            "Updated {} file(s): {}",
            request.changed_files.len(),
            request
                .changed_files
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if added > 0 {
        body.push(format!("Added {added} semantic element(s)."));
    }
    if changed > 0 {
        body.push(format!("Changed {changed} semantic element(s)."));
    }
    if removed > 0 {
        body.push(format!("Removed {removed} semantic element(s)."));
    }
    body.extend(request.changes.iter().take(6).map(describe_change_item));
    if body.is_empty() {
        body.push("No semantic changes were supplied.".to_string());
    }

    SemanticSummaryResponse {
        title,
        body,
        provider,
    }
}

fn heuristic_chat_completion(
    request: &ChatCompletionRequest,
    provider: ReasoningProviderStatus,
) -> ChatCompletionResponse {
    let latest = request
        .messages
        .iter()
        .rev()
        .find(|message| message.role == ChatMessageRole::User)
        .map(|message| message.content.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("your request");
    let context = if request.context.is_empty() {
        "No model context was supplied.".to_string()
    } else {
        format!("I received {} context item(s).", request.context.len())
    };
    ChatCompletionResponse {
        message: format!(
            "I received \"{latest}\". {context} Configure OpenAI or Azure OpenAI in Settings to generate provider-backed answers."
        ),
        provider,
    }
}

pub fn classify_ask_mercurio_task(prompt: &str) -> AskMercurioTask {
    let prompt = prompt.to_ascii_lowercase();
    if is_requirements_view_prompt(&prompt) {
        AskMercurioTask::ViewRequest
    } else if prompt.contains("diagram")
        || prompt.contains("draw")
        || prompt.contains("visual")
        || prompt.contains("graph")
    {
        AskMercurioTask::DiagramRequest
    } else if prompt.contains("proposal")
        || prompt.contains("pull request")
        || prompt.contains(" pr")
        || prompt.contains("pr ")
        || prompt.contains("merge request")
    {
        AskMercurioTask::PrDraft
    } else if prompt.contains("design")
        || prompt.contains("why")
        || prompt.contains("how should")
        || prompt.contains("tradeoff")
        || prompt.contains("architecture")
    {
        AskMercurioTask::DesignQuestion
    } else {
        AskMercurioTask::General
    }
}

fn is_requirements_view_prompt(prompt: &str) -> bool {
    (prompt.contains("requirement") || prompt.contains("requirements"))
        && (prompt.contains("table")
            || prompt.contains("view")
            || prompt.contains("matrix")
            || prompt.contains("show")
            || prompt.contains("list"))
}

fn latest_user_content(messages: &[ChatMessage]) -> &str {
    messages
        .iter()
        .rev()
        .find(|message| message.role == ChatMessageRole::User)
        .map(|message| message.content.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("")
}

fn ask_mercurio_developer_context(task: &AskMercurioTask) -> String {
    let task_detail = match task {
        AskMercurioTask::DesignQuestion => {
            "Answer the design question using only supplied Mercurio project evidence. Cite relevant element or artifact ids."
        }
        AskMercurioTask::DiagramRequest => {
            "Explain the diagram intent briefly. The application may attach a validated diagram_spec artifact separately."
        }
        AskMercurioTask::ViewRequest => {
            "Explain the requested semantic view briefly. The application may attach a validated requirements_view artifact separately."
        }
        AskMercurioTask::PrDraft => {
            "Draft a Mercurio proposal only. Do not claim that branches, commits, files, or pull requests were created."
        }
        AskMercurioTask::General => {
            "Answer as Ask Mercurio for model-aware engineering work. Stay grounded in supplied project evidence."
        }
    };
    format!(
        "You are Ask Mercurio. {task_detail} Be concise, engineering-focused, and explicit when evidence is missing."
    )
}

fn ask_mercurio_citations(
    project: Option<&AskMercurioProjectContext>,
    prompt: &str,
) -> Vec<AskMercurioCitation> {
    let mut citations = Vec::new();
    if let Some(project) = project {
        citations.push(AskMercurioCitation {
            label: project
                .project_name
                .as_deref()
                .unwrap_or(&project.project_id)
                .to_string(),
            target_type: "project".to_string(),
            target_id: project.project_id.clone(),
        });
        if let Some(artifact_id) = &project.artifact_id {
            citations.push(AskMercurioCitation {
                label: "Latest semantic artifact".to_string(),
                target_type: "artifact".to_string(),
                target_id: artifact_id.clone(),
            });
        }
    }
    for token in prompt
        .split_whitespace()
        .filter(|token| token.contains('.'))
        .take(3)
    {
        citations.push(AskMercurioCitation {
            label: token
                .trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '.')
                .to_string(),
            target_type: "element_hint".to_string(),
            target_id: token
                .trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '.')
                .to_string(),
        });
    }
    citations
}

fn ask_mercurio_artifacts(
    task: &AskMercurioTask,
    project: Option<&AskMercurioProjectContext>,
    prompt: &str,
) -> Vec<AskMercurioArtifact> {
    match task {
        AskMercurioTask::DiagramRequest => vec![AskMercurioArtifact::DiagramSpec(json!({
            "version": 1,
            "kind": "dependency_graph",
            "title": diagram_title(prompt),
            "description": "Draft diagram generated from Ask Mercurio request.",
            "root": project.and_then(|project| project.diagram_root_id.as_deref()),
            "rootLabel": project.and_then(|project| project.diagram_root_label.as_deref()),
            "query": {
                "relations": ["specializes", "contains", "references"],
                "direction": "both",
                "depth": 2,
                "include_libraries": false,
                "include_user_model": true
            },
            "layout": {
                "direction": "right"
            },
            "style": {}
        }))],
        AskMercurioTask::ViewRequest => vec![AskMercurioArtifact::RequirementsView(json!({
            "version": 1,
            "kind": "requirements_table",
            "title": requirements_view_title(prompt),
            "description": "Requirements table generated from the current Mercurio semantic graph.",
            "renderer": "table",
            "endpoint": "/api/views/requirements-table"
        }))],
        AskMercurioTask::PrDraft => vec![AskMercurioArtifact::ProposalDraft(ProposalDraft {
            title: pr_title(prompt),
            body: pr_body(project, prompt),
            suggested_base_branch: Some("main".to_string()),
            suggested_head_branch: Some(pr_head_branch(prompt)),
            checklist: vec![
                "Link the proposal to affected semantic elements.".to_string(),
                "Review semantic impact against the latest indexed artifact.".to_string(),
                "Run project validation before preparing source-control changes.".to_string(),
            ],
            linked_semantic_elements: Vec::new(),
        })],
        _ => Vec::new(),
    }
}

fn requirements_view_title(prompt: &str) -> String {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        "Requirements Table".to_string()
    } else {
        format!(
            "Requirements View: {}",
            trimmed.chars().take(56).collect::<String>()
        )
    }
}

fn diagram_title(prompt: &str) -> String {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        "Mercurio Diagram".to_string()
    } else {
        format!("Diagram: {}", trimmed.chars().take(60).collect::<String>())
    }
}

fn pr_title(prompt: &str) -> String {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        "Update Mercurio model".to_string()
    } else {
        format!("Draft: {}", trimmed.chars().take(64).collect::<String>())
    }
}

fn pr_head_branch(prompt: &str) -> String {
    let normalized = prompt
        .split_whitespace()
        .take(6)
        .flat_map(|word| word.chars())
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let slug = normalized
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    format!(
        "ask-mercurio/{}",
        if slug.is_empty() {
            "model-update"
        } else {
            &slug
        }
    )
}

fn pr_body(project: Option<&AskMercurioProjectContext>, prompt: &str) -> String {
    let mut body = String::new();
    body.push_str("## Summary\n");
    body.push_str("- Draft proposal prepared by Ask Mercurio.\n");
    body.push_str("- Requested change: ");
    body.push_str(if prompt.trim().is_empty() {
        "model update"
    } else {
        prompt.trim()
    });
    body.push_str("\n\n## Evidence\n");
    if let Some(project) = project {
        body.push_str(&format!("- Project: {}\n", project.project_id));
        if let Some(artifact_id) = &project.artifact_id {
            body.push_str(&format!("- Semantic artifact: {artifact_id}\n"));
        }
        if let Some(commit) = &project.commit {
            body.push_str(&format!("- Base commit: {commit}\n"));
        }
    } else {
        body.push_str("- No selected project context was attached.\n");
    }
    body.push_str("\n## Validation\n- Run semantic compile and review impact before preparing a branch or PR.\n");
    body
}

fn semantic_summary_title(added: usize, changed: usize, removed: usize) -> String {
    match (added, changed, removed) {
        (0, 0, 0) => "Summarize semantic model state".to_string(),
        (_, 0, 0) if added > 0 => "Add semantic model elements".to_string(),
        (0, _, 0) if changed > 0 => "Update semantic model elements".to_string(),
        (0, 0, _) if removed > 0 => "Remove semantic model elements".to_string(),
        _ => "Update semantic model structure".to_string(),
    }
}

fn describe_change_item(change: &SemanticChangeItem) -> String {
    let label = change.label.as_deref().unwrap_or(&change.element_id);
    let kind = match change.kind {
        SemanticChangeKind::Added => "Added",
        SemanticChangeKind::Removed => "Removed",
        SemanticChangeKind::Changed => "Changed",
        SemanticChangeKind::Unchanged => "Unchanged",
    };
    let mut detail = format!("{kind} {label} ({})", change.element_kind);
    if !change.changed_properties.is_empty() {
        detail.push_str(&format!(
            "; properties: {}",
            change.changed_properties.join(", ")
        ));
    }
    if !change.changed_relationships.is_empty() {
        detail.push_str(&format!(
            "; relationships: {}",
            change.changed_relationships.join(", ")
        ));
    }
    detail
}

fn semantic_summary_developer_prompt() -> &'static str {
    "Write a concise engineering change summary from the supplied semantic diff. \
     Return JSON only. Do not invent changes that are not present. Prefer domain \
     language from element labels and kinds. Keep the title under 72 characters."
}

fn semantic_summary_user_prompt(request: &SemanticSummaryRequest) -> String {
    serde_json::to_string_pretty(request).unwrap_or_else(|_| "{}".to_string())
}

fn extract_output_text(response: &OpenAiStructuredResponse) -> Result<String, String> {
    for output in &response.output {
        for content in &output.content {
            match content {
                OpenAiContentItem::OutputText { text } => return Ok(text.clone()),
                OpenAiContentItem::Refusal { refusal } => {
                    return Err(format!("model refused structured response: {refusal}"));
                }
                OpenAiContentItem::Other => {}
            }
        }
    }

    Err("no output_text item found in AI provider response".to_string())
}

fn semantic_summary_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "title": { "type": "string" },
            "body": {
                "type": "array",
                "items": { "type": "string" }
            }
        },
        "required": ["title", "body"]
    })
}

fn connection_probe_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "ok": { "type": "boolean" }
        },
        "required": ["ok"]
    })
}

fn normalize_azure_openai_base_url(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.ends_with("/openai/v1/responses") {
        return trimmed.to_string();
    }
    if trimmed.ends_with("/openai/v1") {
        return format!("{trimmed}/responses");
    }
    format!("{trimmed}{DEFAULT_AZURE_OPENAI_PATH}")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        OpenAiStructuredResponse, ReasoningProviderConfigOverrides,
        ReasoningProviderSecretOverrides, SemanticChangeItem, SemanticChangeKind,
        SemanticSummaryRequest, ask_mercurio_artifacts, classify_ask_mercurio_task,
        extract_output_text, heuristic_provider, normalize_azure_openai_base_url,
        summarize_semantic_changes, test_configured_reasoning_provider_connection,
    };
    use crate::ai::{
        AskMercurioArtifact, AskMercurioTask, ReasoningProvider, ReasoningProviderKind,
    };

    #[test]
    fn heuristic_summary_counts_semantic_changes() {
        let response = summarize_semantic_changes(&SemanticSummaryRequest {
            title_hint: None,
            changed_files: vec!["models/vehicle.sysml".to_string()],
            changes: vec![SemanticChangeItem {
                kind: SemanticChangeKind::Added,
                element_id: "type.Vehicle.Battery".to_string(),
                element_kind: "PartDefinition".to_string(),
                label: Some("Battery".to_string()),
                changed_properties: Vec::new(),
                changed_relationships: Vec::new(),
                source_path: Some("models/vehicle.sysml".to_string()),
            }],
        });

        assert_eq!(response.title, "Add semantic model elements");
        assert!(response.body.iter().any(|line| line.contains("Added 1")));
    }

    #[test]
    fn heuristic_provider_is_always_testable() {
        let provider = heuristic_provider();
        let status = provider.test_connection().unwrap();
        assert!(status.structured_outputs);
    }

    #[test]
    fn configured_azure_test_does_not_fall_back_to_heuristic() {
        let result = test_configured_reasoning_provider_connection(
            ReasoningProviderConfigOverrides {
                provider: Some(ReasoningProviderKind::AzureOpenAi),
                azure_openai_deployment: Some("test-mini".to_string()),
                azure_openai_base_url: Some("https://example.openai.azure.com".to_string()),
                ..ReasoningProviderConfigOverrides::default()
            },
            ReasoningProviderSecretOverrides::default(),
        );

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Azure OpenAI settings are incomplete")
        );
    }

    #[test]
    fn extract_output_text_reads_structured_response() {
        let response: OpenAiStructuredResponse = serde_json::from_value(json!({
            "output": [
                {
                    "content": [
                        {
                            "type": "output_text",
                            "text": "{\"title\":\"ok\",\"body\":[\"careful\"]}"
                        }
                    ]
                }
            ]
        }))
        .unwrap();

        let output = extract_output_text(&response).unwrap();
        assert!(output.contains("\"title\""));
    }

    #[test]
    fn normalize_azure_openai_base_url_accepts_endpoint_or_v1_base() {
        assert_eq!(
            normalize_azure_openai_base_url("https://example.openai.azure.com"),
            "https://example.openai.azure.com/openai/v1/responses"
        );
        assert_eq!(
            normalize_azure_openai_base_url("https://example.openai.azure.com/openai/v1/"),
            "https://example.openai.azure.com/openai/v1/responses"
        );
        assert_eq!(
            normalize_azure_openai_base_url("https://example.openai.azure.com/openai/v1/responses"),
            "https://example.openai.azure.com/openai/v1/responses"
        );
    }

    #[test]
    fn ask_mercurio_classifies_supported_tasks() {
        assert_eq!(
            classify_ask_mercurio_task("Create a dependency diagram for the camera model"),
            AskMercurioTask::DiagramRequest
        );
        assert_eq!(
            classify_ask_mercurio_task("Create a requirements table view"),
            AskMercurioTask::ViewRequest
        );
        assert_eq!(
            classify_ask_mercurio_task("Draft a pull request for this update"),
            AskMercurioTask::PrDraft
        );
        assert_eq!(
            classify_ask_mercurio_task("What design tradeoff is represented here?"),
            AskMercurioTask::DesignQuestion
        );
    }

    #[test]
    fn ask_mercurio_pr_task_returns_draft_only_artifact() {
        let artifacts = ask_mercurio_artifacts(
            &AskMercurioTask::PrDraft,
            None,
            "Draft a proposal for a brake model update",
        );

        let Some(AskMercurioArtifact::ProposalDraft(draft)) = artifacts.first() else {
            panic!("expected proposal draft artifact");
        };
        assert!(draft.title.contains("Draft:"));
        assert!(
            draft
                .suggested_head_branch
                .as_deref()
                .unwrap_or_default()
                .starts_with("ask-mercurio/")
        );
        assert!(draft.body.contains("No selected project context"));
        assert!(
            draft
                .checklist
                .iter()
                .any(|item| item.contains("semantic impact"))
        );
    }

    #[test]
    fn ask_mercurio_view_task_returns_requirements_view_artifact() {
        let artifacts = ask_mercurio_artifacts(
            &AskMercurioTask::ViewRequest,
            None,
            "Show me a requirements table",
        );

        let Some(AskMercurioArtifact::RequirementsView(view)) = artifacts.first() else {
            panic!("expected requirements view artifact");
        };
        assert_eq!(view["kind"], "requirements_table");
        assert_eq!(view["endpoint"], "/api/views/requirements-table");
    }
}
