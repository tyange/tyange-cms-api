use std::{env, sync::Arc};

use reqwest::{
    header::{ACCEPT, AUTHORIZATION, USER_AGENT},
    Client, StatusCode,
};
use serde::Serialize;

const DEFAULT_API_BASE: &str = "https://api.github.com";
const DEFAULT_REPO: &str = "tyange/tyange-blog";
const DEFAULT_EVENT_TYPE: &str = "cms-content-changed";
const SOURCE_NAME: &str = "tyange-cms-api";
const GITHUB_API_VERSION: &str = "2022-11-28";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlogContentEvent {
    Publish,
    Update,
    Delete,
}

impl BlogContentEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Publish => "publish",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlogVisibility {
    Visible,
    Hidden,
}

impl BlogVisibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::Hidden => "hidden",
        }
    }
}

#[derive(Clone)]
pub struct BlogRedeployService {
    client: Client,
    mode: BlogRedeployMode,
}

#[derive(Clone)]
enum BlogRedeployMode {
    Disabled {
        reason: Arc<str>,
    },
    GitHub {
        config: Arc<BlogRedeployConfig>,
    },
    #[cfg(test)]
    Mock {
        handle: Arc<MockBlogRedeployHandle>,
    },
}

#[derive(Clone, Debug)]
struct BlogRedeployConfig {
    api_base: String,
    repo: String,
    event_type: String,
    token: String,
}

#[derive(Debug, Serialize)]
struct RepositoryDispatchRequest<'a> {
    event_type: &'a str,
    client_payload: RepositoryDispatchPayload<'a>,
}

#[derive(Debug, Serialize)]
struct RepositoryDispatchPayload<'a> {
    source: &'a str,
    content_event: &'a str,
    post_id: &'a str,
    visibility: &'a str,
}

#[derive(Debug)]
struct DispatchFailureLog {
    content_event: BlogContentEvent,
    post_id: String,
    visibility: BlogVisibility,
    status: Option<StatusCode>,
    message: String,
}

impl BlogRedeployService {
    pub fn from_env() -> Self {
        let client = Client::new();
        let config = BlogRedeployConfig::from_env();

        let mode = match config {
            Some(config) => BlogRedeployMode::GitHub {
                config: Arc::new(config),
            },
            None => BlogRedeployMode::Disabled {
                reason: Arc::from(
                    "TYANGE_BLOG_REDEPLOY_TOKEN 환경변수가 비어 있어 blog rebuild trigger를 건너뜁니다.",
                ),
            },
        };

        Self { client, mode }
    }

    pub async fn dispatch_content_change(
        &self,
        content_event: BlogContentEvent,
        post_id: &str,
        visibility: BlogVisibility,
    ) {
        match &self.mode {
            BlogRedeployMode::Disabled { reason } => {
                self.log_failure(DispatchFailureLog {
                    content_event,
                    post_id: post_id.to_string(),
                    visibility,
                    status: None,
                    message: reason.to_string(),
                });
            }
            BlogRedeployMode::GitHub { config } => {
                let payload = RepositoryDispatchRequest {
                    event_type: &config.event_type,
                    client_payload: RepositoryDispatchPayload {
                        source: SOURCE_NAME,
                        content_event: content_event.as_str(),
                        post_id,
                        visibility: visibility.as_str(),
                    },
                };

                let url = format!(
                    "{}/repos/{}/dispatches",
                    config.api_base.trim_end_matches('/'),
                    config.repo
                );

                match self
                    .client
                    .post(url)
                    .header(ACCEPT, "application/vnd.github+json")
                    .header(AUTHORIZATION, format!("Bearer {}", config.token))
                    .header("X-GitHub-Api-Version", GITHUB_API_VERSION)
                    .header(USER_AGENT, SOURCE_NAME)
                    .json(&payload)
                    .send()
                    .await
                {
                    Ok(response) if response.status() == StatusCode::NO_CONTENT => {
                        println!(
                            "blog redeploy dispatch accepted: content_event={}, post_id={}, visibility={}, github_status={}",
                            content_event.as_str(),
                            post_id,
                            visibility.as_str(),
                            response.status().as_u16()
                        );
                    }
                    Ok(response) => {
                        let status = response.status();
                        let body = response.text().await.unwrap_or_default();
                        self.log_failure(DispatchFailureLog {
                            content_event,
                            post_id: post_id.to_string(),
                            visibility,
                            status: Some(status),
                            message: if body.is_empty() {
                                "GitHub repository_dispatch 호출이 204를 반환하지 않았습니다."
                                    .to_string()
                            } else {
                                format!(
                                    "GitHub repository_dispatch 호출이 204를 반환하지 않았습니다. body={}",
                                    body
                                )
                            },
                        });
                    }
                    Err(error) => {
                        self.log_failure(DispatchFailureLog {
                            content_event,
                            post_id: post_id.to_string(),
                            visibility,
                            status: None,
                            message: error.to_string(),
                        });
                    }
                }
            }
            #[cfg(test)]
            BlogRedeployMode::Mock { handle } => {
                handle
                    .dispatch(content_event, post_id.to_string(), visibility)
                    .await;
            }
        }
    }

    fn log_failure(&self, failure: DispatchFailureLog) {
        eprintln!(
            "blog redeploy dispatch failed: content_event={}, post_id={}, visibility={}, github_status={}, error={}",
            failure.content_event.as_str(),
            failure.post_id,
            failure.visibility.as_str(),
            failure
                .status
                .map(|status| status.as_u16().to_string())
                .unwrap_or_else(|| "none".to_string()),
            failure.message
        );
    }

    #[cfg(test)]
    pub fn mock() -> (Self, Arc<MockBlogRedeployHandle>) {
        let handle = Arc::new(MockBlogRedeployHandle::default());
        let service = Self {
            client: Client::new(),
            mode: BlogRedeployMode::Mock {
                handle: handle.clone(),
            },
        };

        (service, handle)
    }
}

impl BlogRedeployConfig {
    fn from_env() -> Option<Self> {
        let token = env::var("TYANGE_BLOG_REDEPLOY_TOKEN").ok()?;
        let token = token.trim().to_string();
        if token.is_empty() {
            return None;
        }

        let repo = env::var("TYANGE_BLOG_REDEPLOY_REPO")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_REPO.to_string());

        let event_type = env::var("TYANGE_BLOG_REDEPLOY_EVENT_TYPE")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_EVENT_TYPE.to_string());

        let api_base = env::var("TYANGE_BLOG_REDEPLOY_API_BASE")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_API_BASE.to_string());

        Some(Self {
            api_base,
            repo,
            event_type,
            token,
        })
    }
}

pub fn is_publicly_visible(status: &str) -> bool {
    !status.trim().eq_ignore_ascii_case("draft")
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MockDispatchCall {
    pub content_event: BlogContentEvent,
    pub post_id: String,
    pub visibility: BlogVisibility,
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MockDispatchFailure {
    pub content_event: BlogContentEvent,
    pub post_id: String,
    pub visibility: BlogVisibility,
    pub status: Option<u16>,
    pub message: String,
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct MockFailurePlan {
    status: Option<u16>,
    message: String,
}

#[cfg(test)]
#[derive(Default)]
pub struct MockBlogRedeployHandle {
    calls: tokio::sync::Mutex<Vec<MockDispatchCall>>,
    failures: tokio::sync::Mutex<Vec<MockDispatchFailure>>,
    next_failure: tokio::sync::Mutex<Option<MockFailurePlan>>,
}

#[cfg(test)]
impl MockBlogRedeployHandle {
    async fn dispatch(
        &self,
        content_event: BlogContentEvent,
        post_id: String,
        visibility: BlogVisibility,
    ) {
        self.calls.lock().await.push(MockDispatchCall {
            content_event,
            post_id: post_id.clone(),
            visibility,
        });

        if let Some(failure_plan) = self.next_failure.lock().await.take() {
            let failure = MockDispatchFailure {
                content_event,
                post_id: post_id.clone(),
                visibility,
                status: failure_plan.status,
                message: failure_plan.message,
            };
            let status_text = failure
                .status
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string());
            let message = failure.message.clone();
            self.failures.lock().await.push(failure);
            eprintln!(
                "blog redeploy dispatch failed: content_event={}, post_id={}, visibility={}, github_status={}, error={}",
                content_event.as_str(),
                post_id,
                visibility.as_str(),
                status_text,
                message
            );
        }
    }

    pub async fn take_calls(&self) -> Vec<MockDispatchCall> {
        std::mem::take(&mut *self.calls.lock().await)
    }

    pub async fn take_failures(&self) -> Vec<MockDispatchFailure> {
        std::mem::take(&mut *self.failures.lock().await)
    }

    pub async fn fail_next(&self, failure: MockDispatchFailure) {
        *self.next_failure.lock().await = Some(MockFailurePlan {
            status: failure.status,
            message: failure.message,
        });
    }
}
