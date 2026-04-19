#![allow(unused_imports, unused_macros)]

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "e2b")]
pub const DEFAULT_E2B_WORKSPACE_ROOT: &str = "/workspace";
#[cfg(feature = "modal")]
pub const DEFAULT_MODAL_WORKSPACE_ROOT: &str = "/workspace";
#[cfg(feature = "daytona")]
pub const DEFAULT_DAYTONA_WORKSPACE_ROOT: &str = "/home/daytona/workspace";
#[cfg(feature = "blaxel")]
pub const DEFAULT_BLAXEL_WORKSPACE_ROOT: &str = "/workspace";
#[cfg(feature = "cloudflare")]
pub const DEFAULT_CLOUDFLARE_WORKSPACE_ROOT: &str = "/workspace";
#[cfg(feature = "runloop")]
pub const DEFAULT_RUNLOOP_WORKSPACE_ROOT: &str = "/home/user";
#[cfg(feature = "vercel")]
pub const DEFAULT_VERCEL_WORKSPACE_ROOT: &str = "/vercel/sandbox";

#[cfg(any(
    feature = "e2b",
    feature = "modal",
    feature = "daytona",
    feature = "blaxel",
    feature = "cloudflare",
    feature = "runloop",
    feature = "vercel"
))]
static NEXT_HOSTED_SESSION_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(any(
    feature = "e2b",
    feature = "modal",
    feature = "daytona",
    feature = "blaxel",
    feature = "cloudflare",
    feature = "runloop",
    feature = "vercel"
))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HostedAuthKind {
    ApiKey,
    Token,
}

#[cfg(any(
    feature = "e2b",
    feature = "modal",
    feature = "daytona",
    feature = "blaxel",
    feature = "cloudflare",
    feature = "runloop",
    feature = "vercel"
))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkspaceRootPolicy {
    Strict(&'static str),
    Defaulted(&'static str),
}

#[cfg(any(
    feature = "e2b",
    feature = "modal",
    feature = "daytona",
    feature = "blaxel",
    feature = "cloudflare",
    feature = "runloop",
    feature = "vercel"
))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HostedProviderSpec {
    provider_name: &'static str,
    auth_kind: HostedAuthKind,
    auth_env_var: &'static str,
    workspace_root_policy: WorkspaceRootPolicy,
    supports_exposed_ports: bool,
    supports_pty: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostedSandboxError {
    message: String,
}

impl HostedSandboxError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for HostedSandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for HostedSandboxError {}

type HostedSandboxResult<T> = std::result::Result<T, HostedSandboxError>;

fn default_read_only() -> bool {
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HostedBucketProvider {
    S3,
    R2,
    Gcs,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HostedAccessKeyCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum HostedMountStrategy {
    #[serde(rename = "e2b_cloud_bucket")]
    E2bCloudBucket,
    #[serde(rename = "modal_cloud_bucket")]
    ModalCloudBucket {
        #[serde(default)]
        secret_name: Option<String>,
        #[serde(default)]
        secret_environment_name: Option<String>,
    },
    #[serde(rename = "daytona_cloud_bucket")]
    DaytonaCloudBucket,
    #[serde(rename = "blaxel_cloud_bucket")]
    BlaxelCloudBucket,
    #[serde(rename = "cloudflare_bucket_mount")]
    CloudflareBucketMount,
    #[serde(rename = "runloop_cloud_bucket")]
    RunloopCloudBucket,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HostedS3Mount {
    pub bucket: String,
    #[serde(default)]
    pub access_key_id: Option<String>,
    #[serde(default)]
    pub secret_access_key: Option<String>,
    #[serde(default)]
    pub session_token: Option<String>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub endpoint_url: Option<String>,
    #[serde(default = "default_read_only")]
    pub read_only: bool,
    #[serde(default)]
    pub mount_path: Option<String>,
    pub mount_strategy: HostedMountStrategy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HostedR2Mount {
    pub bucket: String,
    pub account_id: String,
    #[serde(default)]
    pub access_key_id: Option<String>,
    #[serde(default)]
    pub secret_access_key: Option<String>,
    #[serde(default)]
    pub custom_domain: Option<String>,
    #[serde(default = "default_read_only")]
    pub read_only: bool,
    #[serde(default)]
    pub mount_path: Option<String>,
    pub mount_strategy: HostedMountStrategy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HostedGcsMount {
    pub bucket: String,
    #[serde(default)]
    pub access_id: Option<String>,
    #[serde(default)]
    pub secret_access_key: Option<String>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub endpoint_url: Option<String>,
    #[serde(default)]
    pub service_account_credentials: Option<String>,
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default = "default_read_only")]
    pub read_only: bool,
    #[serde(default)]
    pub mount_path: Option<String>,
    pub mount_strategy: HostedMountStrategy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum HostedMountEntry {
    #[serde(rename = "s3_mount")]
    S3Mount(HostedS3Mount),
    #[serde(rename = "r2_mount")]
    R2Mount(HostedR2Mount),
    #[serde(rename = "gcs_mount")]
    GcsMount(HostedGcsMount),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HostedRcloneMountPayload {
    pub provider: HostedBucketProvider,
    pub strategy: String,
    pub bucket: String,
    pub remote_path: String,
    pub mount_path: String,
    #[serde(default)]
    pub endpoint_url: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub credentials: Option<HostedAccessKeyCredentials>,
    #[serde(default)]
    pub session_token: Option<String>,
    #[serde(default)]
    pub service_account_credentials: Option<String>,
    #[serde(default)]
    pub access_token: Option<String>,
    pub read_only: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModalCloudBucketMountPayload {
    pub bucket_name: String,
    #[serde(default)]
    pub bucket_endpoint_url: Option<String>,
    #[serde(default)]
    pub key_prefix: Option<String>,
    #[serde(default)]
    pub credentials: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(default)]
    pub secret_name: Option<String>,
    #[serde(default)]
    pub secret_environment_name: Option<String>,
    pub read_only: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BlaxelCloudBucketMountPayload {
    pub provider: HostedBucketProvider,
    pub bucket: String,
    pub mount_path: String,
    pub read_only: bool,
    #[serde(default)]
    pub access_key_id: Option<String>,
    #[serde(default)]
    pub secret_access_key: Option<String>,
    #[serde(default)]
    pub session_token: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub endpoint_url: Option<String>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub service_account_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CloudflareBucketMountPayload {
    pub bucket_name: String,
    pub bucket_endpoint_url: String,
    pub provider: HostedBucketProvider,
    #[serde(default)]
    pub key_prefix: Option<String>,
    #[serde(default)]
    pub credentials: Option<HostedAccessKeyCredentials>,
    pub read_only: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "provider")]
pub enum HostedProviderMountPayload {
    #[serde(rename = "e2b")]
    E2b { config: HostedRcloneMountPayload },
    #[serde(rename = "modal")]
    Modal {
        config: ModalCloudBucketMountPayload,
    },
    #[serde(rename = "daytona")]
    Daytona { config: HostedRcloneMountPayload },
    #[serde(rename = "blaxel")]
    Blaxel {
        config: BlaxelCloudBucketMountPayload,
    },
    #[serde(rename = "cloudflare")]
    Cloudflare {
        config: CloudflareBucketMountPayload,
    },
    #[serde(rename = "runloop")]
    Runloop { config: HostedRcloneMountPayload },
}

impl HostedMountEntry {
    pub fn resolve_provider_payload(&self) -> HostedSandboxResult<HostedProviderMountPayload> {
        match self {
            Self::S3Mount(mount) => mount.mount_strategy.resolve_s3_payload(mount),
            Self::R2Mount(mount) => mount.mount_strategy.resolve_r2_payload(mount),
            Self::GcsMount(mount) => mount.mount_strategy.resolve_gcs_payload(mount),
        }
    }
}

impl HostedMountStrategy {
    fn resolve_s3_payload(
        &self,
        mount: &HostedS3Mount,
    ) -> HostedSandboxResult<HostedProviderMountPayload> {
        validate_access_key_pair(
            "s3 mounts",
            mount.access_key_id.as_deref(),
            mount.secret_access_key.as_deref(),
        )?;
        if mount.session_token.is_some()
            && (mount.access_key_id.is_none() || mount.secret_access_key.is_none())
        {
            return Err(HostedSandboxError::new(
                "s3 mounts require access_key_id and secret_access_key when session_token is set",
            ));
        }

        match self {
            Self::E2bCloudBucket => Ok(HostedProviderMountPayload::E2b {
                config: build_rclone_s3_payload("e2b_cloud_bucket", mount),
            }),
            Self::ModalCloudBucket {
                secret_name,
                secret_environment_name,
            } => Ok(HostedProviderMountPayload::Modal {
                config: build_modal_s3_payload(
                    mount,
                    secret_name.clone(),
                    secret_environment_name.clone(),
                )?,
            }),
            Self::DaytonaCloudBucket => Ok(HostedProviderMountPayload::Daytona {
                config: build_rclone_s3_payload("daytona_cloud_bucket", mount),
            }),
            Self::BlaxelCloudBucket => Ok(HostedProviderMountPayload::Blaxel {
                config: build_blaxel_s3_payload(mount),
            }),
            Self::CloudflareBucketMount => Ok(HostedProviderMountPayload::Cloudflare {
                config: build_cloudflare_s3_payload(mount)?,
            }),
            Self::RunloopCloudBucket => Ok(HostedProviderMountPayload::Runloop {
                config: build_rclone_s3_payload("runloop_cloud_bucket", mount),
            }),
        }
    }

    fn resolve_r2_payload(
        &self,
        mount: &HostedR2Mount,
    ) -> HostedSandboxResult<HostedProviderMountPayload> {
        validate_access_key_pair(
            "r2 credentials",
            mount.access_key_id.as_deref(),
            mount.secret_access_key.as_deref(),
        )?;

        match self {
            Self::E2bCloudBucket => Ok(HostedProviderMountPayload::E2b {
                config: build_rclone_r2_payload("e2b_cloud_bucket", mount),
            }),
            Self::ModalCloudBucket {
                secret_name,
                secret_environment_name,
            } => Ok(HostedProviderMountPayload::Modal {
                config: build_modal_r2_payload(
                    mount,
                    secret_name.clone(),
                    secret_environment_name.clone(),
                )?,
            }),
            Self::DaytonaCloudBucket => Ok(HostedProviderMountPayload::Daytona {
                config: build_rclone_r2_payload("daytona_cloud_bucket", mount),
            }),
            Self::BlaxelCloudBucket => Ok(HostedProviderMountPayload::Blaxel {
                config: build_blaxel_r2_payload(mount),
            }),
            Self::CloudflareBucketMount => Ok(HostedProviderMountPayload::Cloudflare {
                config: build_cloudflare_r2_payload(mount),
            }),
            Self::RunloopCloudBucket => Ok(HostedProviderMountPayload::Runloop {
                config: build_rclone_r2_payload("runloop_cloud_bucket", mount),
            }),
        }
    }

    fn resolve_gcs_payload(
        &self,
        mount: &HostedGcsMount,
    ) -> HostedSandboxResult<HostedProviderMountPayload> {
        validate_access_key_pair(
            "gcs hmac credentials",
            mount.access_id.as_deref(),
            mount.secret_access_key.as_deref(),
        )?;

        match self {
            Self::E2bCloudBucket => Ok(HostedProviderMountPayload::E2b {
                config: build_rclone_gcs_payload("e2b_cloud_bucket", mount),
            }),
            Self::ModalCloudBucket {
                secret_name,
                secret_environment_name,
            } => Ok(HostedProviderMountPayload::Modal {
                config: build_modal_gcs_payload(
                    mount,
                    secret_name.clone(),
                    secret_environment_name.clone(),
                )?,
            }),
            Self::DaytonaCloudBucket => Ok(HostedProviderMountPayload::Daytona {
                config: build_rclone_gcs_payload("daytona_cloud_bucket", mount),
            }),
            Self::BlaxelCloudBucket => Ok(HostedProviderMountPayload::Blaxel {
                config: build_blaxel_gcs_payload(mount)?,
            }),
            Self::CloudflareBucketMount => Ok(HostedProviderMountPayload::Cloudflare {
                config: build_cloudflare_gcs_payload(mount)?,
            }),
            Self::RunloopCloudBucket => Ok(HostedProviderMountPayload::Runloop {
                config: build_rclone_gcs_payload("runloop_cloud_bucket", mount),
            }),
        }
    }
}

fn build_rclone_s3_payload(strategy: &str, mount: &HostedS3Mount) -> HostedRcloneMountPayload {
    HostedRcloneMountPayload {
        provider: HostedBucketProvider::S3,
        strategy: strategy.to_owned(),
        bucket: mount.bucket.clone(),
        remote_path: join_remote_path(&mount.bucket, mount.prefix.as_deref()),
        mount_path: mount
            .mount_path
            .clone()
            .unwrap_or_else(|| "/workspace".to_owned()),
        endpoint_url: mount.endpoint_url.clone(),
        region: mount.region.clone(),
        credentials: build_access_key_credentials(
            mount.access_key_id.as_deref(),
            mount.secret_access_key.as_deref(),
        ),
        session_token: mount.session_token.clone(),
        service_account_credentials: None,
        access_token: None,
        read_only: mount.read_only,
    }
}

fn build_rclone_r2_payload(strategy: &str, mount: &HostedR2Mount) -> HostedRcloneMountPayload {
    HostedRcloneMountPayload {
        provider: HostedBucketProvider::R2,
        strategy: strategy.to_owned(),
        bucket: mount.bucket.clone(),
        remote_path: mount.bucket.clone(),
        mount_path: mount
            .mount_path
            .clone()
            .unwrap_or_else(|| "/workspace".to_owned()),
        endpoint_url: Some(r2_endpoint(
            &mount.account_id,
            mount.custom_domain.as_deref(),
        )),
        region: None,
        credentials: build_access_key_credentials(
            mount.access_key_id.as_deref(),
            mount.secret_access_key.as_deref(),
        ),
        session_token: None,
        service_account_credentials: None,
        access_token: None,
        read_only: mount.read_only,
    }
}

fn build_rclone_gcs_payload(strategy: &str, mount: &HostedGcsMount) -> HostedRcloneMountPayload {
    HostedRcloneMountPayload {
        provider: HostedBucketProvider::Gcs,
        strategy: strategy.to_owned(),
        bucket: mount.bucket.clone(),
        remote_path: join_remote_path(&mount.bucket, mount.prefix.as_deref()),
        mount_path: mount
            .mount_path
            .clone()
            .unwrap_or_else(|| "/workspace".to_owned()),
        endpoint_url: Some(
            mount
                .endpoint_url
                .clone()
                .unwrap_or_else(|| "https://storage.googleapis.com".to_owned()),
        ),
        region: mount.region.clone(),
        credentials: build_access_key_credentials(
            mount.access_id.as_deref(),
            mount.secret_access_key.as_deref(),
        ),
        session_token: None,
        service_account_credentials: mount.service_account_credentials.clone(),
        access_token: mount.access_token.clone(),
        read_only: mount.read_only,
    }
}

fn build_modal_s3_payload(
    mount: &HostedS3Mount,
    secret_name: Option<String>,
    secret_environment_name: Option<String>,
) -> HostedSandboxResult<ModalCloudBucketMountPayload> {
    validate_modal_secret_fields(
        secret_name.as_deref(),
        secret_environment_name.as_deref(),
        "s3_mount",
    )?;
    if secret_name.is_some()
        && (mount.access_key_id.is_some()
            || mount.secret_access_key.is_some()
            || mount.session_token.is_some())
    {
        return Err(HostedSandboxError::new(
            "modal cloud bucket mounts do not support both inline credentials and secret_name",
        ));
    }

    let mut credentials = serde_json::Map::new();
    if let Some(value) = &mount.access_key_id {
        credentials.insert(
            "AWS_ACCESS_KEY_ID".to_owned(),
            serde_json::Value::String(value.clone()),
        );
    }
    if let Some(value) = &mount.secret_access_key {
        credentials.insert(
            "AWS_SECRET_ACCESS_KEY".to_owned(),
            serde_json::Value::String(value.clone()),
        );
    }
    if let Some(value) = &mount.session_token {
        credentials.insert(
            "AWS_SESSION_TOKEN".to_owned(),
            serde_json::Value::String(value.clone()),
        );
    }

    Ok(ModalCloudBucketMountPayload {
        bucket_name: mount.bucket.clone(),
        bucket_endpoint_url: mount.endpoint_url.clone(),
        key_prefix: mount.prefix.clone(),
        credentials: (!credentials.is_empty()).then_some(credentials),
        secret_name,
        secret_environment_name,
        read_only: mount.read_only,
    })
}

fn build_modal_r2_payload(
    mount: &HostedR2Mount,
    secret_name: Option<String>,
    secret_environment_name: Option<String>,
) -> HostedSandboxResult<ModalCloudBucketMountPayload> {
    validate_modal_secret_fields(
        secret_name.as_deref(),
        secret_environment_name.as_deref(),
        "r2_mount",
    )?;
    if secret_name.is_some() && mount.access_key_id.is_some() {
        return Err(HostedSandboxError::new(
            "modal cloud bucket mounts do not support both inline credentials and secret_name",
        ));
    }

    let mut credentials = serde_json::Map::new();
    if let Some(value) = &mount.access_key_id {
        credentials.insert(
            "AWS_ACCESS_KEY_ID".to_owned(),
            serde_json::Value::String(value.clone()),
        );
    }
    if let Some(value) = &mount.secret_access_key {
        credentials.insert(
            "AWS_SECRET_ACCESS_KEY".to_owned(),
            serde_json::Value::String(value.clone()),
        );
    }

    Ok(ModalCloudBucketMountPayload {
        bucket_name: mount.bucket.clone(),
        bucket_endpoint_url: Some(r2_endpoint(
            &mount.account_id,
            mount.custom_domain.as_deref(),
        )),
        key_prefix: None,
        credentials: (!credentials.is_empty()).then_some(credentials),
        secret_name,
        secret_environment_name,
        read_only: mount.read_only,
    })
}

fn build_modal_gcs_payload(
    mount: &HostedGcsMount,
    secret_name: Option<String>,
    secret_environment_name: Option<String>,
) -> HostedSandboxResult<ModalCloudBucketMountPayload> {
    validate_modal_secret_fields(
        secret_name.as_deref(),
        secret_environment_name.as_deref(),
        "gcs_mount",
    )?;
    let using_hmac = mount.access_id.is_some() && mount.secret_access_key.is_some();
    if !using_hmac && secret_name.is_none() {
        return Err(HostedSandboxError::new(
            "gcs modal cloud bucket mounts require access_id and secret_access_key",
        ));
    }
    if secret_name.is_some()
        && (mount.access_id.is_some()
            || mount.secret_access_key.is_some()
            || mount.service_account_credentials.is_some()
            || mount.access_token.is_some())
    {
        return Err(HostedSandboxError::new(
            "modal cloud bucket mounts do not support both inline credentials and secret_name",
        ));
    }
    if mount.service_account_credentials.is_some() || mount.access_token.is_some() {
        return Err(HostedSandboxError::new(
            "gcs modal cloud bucket mounts require access_id and secret_access_key",
        ));
    }

    let credentials = using_hmac.then(|| {
        let mut map = serde_json::Map::new();
        map.insert(
            "GOOGLE_ACCESS_KEY_ID".to_owned(),
            serde_json::Value::String(mount.access_id.clone().expect("access_id should exist")),
        );
        map.insert(
            "GOOGLE_ACCESS_KEY_SECRET".to_owned(),
            serde_json::Value::String(
                mount
                    .secret_access_key
                    .clone()
                    .expect("secret_access_key should exist"),
            ),
        );
        map
    });

    Ok(ModalCloudBucketMountPayload {
        bucket_name: mount.bucket.clone(),
        bucket_endpoint_url: Some(
            mount
                .endpoint_url
                .clone()
                .unwrap_or_else(|| "https://storage.googleapis.com".to_owned()),
        ),
        key_prefix: mount.prefix.clone(),
        credentials,
        secret_name,
        secret_environment_name,
        read_only: mount.read_only,
    })
}

fn build_blaxel_s3_payload(mount: &HostedS3Mount) -> BlaxelCloudBucketMountPayload {
    BlaxelCloudBucketMountPayload {
        provider: HostedBucketProvider::S3,
        bucket: mount.bucket.clone(),
        mount_path: mount
            .mount_path
            .clone()
            .unwrap_or_else(|| "/workspace".to_owned()),
        read_only: mount.read_only,
        access_key_id: mount.access_key_id.clone(),
        secret_access_key: mount.secret_access_key.clone(),
        session_token: mount.session_token.clone(),
        region: mount.region.clone(),
        endpoint_url: mount.endpoint_url.clone(),
        prefix: mount.prefix.clone(),
        service_account_key: None,
    }
}

fn build_blaxel_r2_payload(mount: &HostedR2Mount) -> BlaxelCloudBucketMountPayload {
    BlaxelCloudBucketMountPayload {
        provider: HostedBucketProvider::R2,
        bucket: mount.bucket.clone(),
        mount_path: mount
            .mount_path
            .clone()
            .unwrap_or_else(|| "/workspace".to_owned()),
        read_only: mount.read_only,
        access_key_id: mount.access_key_id.clone(),
        secret_access_key: mount.secret_access_key.clone(),
        session_token: None,
        region: None,
        endpoint_url: Some(r2_endpoint(
            &mount.account_id,
            mount.custom_domain.as_deref(),
        )),
        prefix: None,
        service_account_key: None,
    }
}

fn build_blaxel_gcs_payload(
    mount: &HostedGcsMount,
) -> HostedSandboxResult<BlaxelCloudBucketMountPayload> {
    let using_hmac = mount.access_id.is_some() && mount.secret_access_key.is_some();
    if using_hmac && mount.service_account_credentials.is_some() {
        return Err(HostedSandboxError::new(
            "blaxel cloud bucket mounts do not support both hmac and service_account_credentials",
        ));
    }
    if mount.access_token.is_some() {
        return Err(HostedSandboxError::new(
            "blaxel cloud bucket mounts do not support gcs access_token credentials",
        ));
    }

    Ok(BlaxelCloudBucketMountPayload {
        provider: if using_hmac {
            HostedBucketProvider::S3
        } else {
            HostedBucketProvider::Gcs
        },
        bucket: mount.bucket.clone(),
        mount_path: mount
            .mount_path
            .clone()
            .unwrap_or_else(|| "/workspace".to_owned()),
        read_only: mount.read_only,
        access_key_id: mount.access_id.clone(),
        secret_access_key: mount.secret_access_key.clone(),
        session_token: None,
        region: mount.region.clone(),
        endpoint_url: if using_hmac {
            Some(
                mount
                    .endpoint_url
                    .clone()
                    .unwrap_or_else(|| "https://storage.googleapis.com".to_owned()),
            )
        } else {
            None
        },
        prefix: mount.prefix.clone(),
        service_account_key: if using_hmac {
            None
        } else {
            mount.service_account_credentials.clone()
        },
    })
}

fn build_cloudflare_s3_payload(
    mount: &HostedS3Mount,
) -> HostedSandboxResult<CloudflareBucketMountPayload> {
    if mount.session_token.is_some() {
        return Err(HostedSandboxError::new(
            "cloudflare bucket mounts do not support s3 session_token credentials",
        ));
    }
    Ok(CloudflareBucketMountPayload {
        bucket_name: mount.bucket.clone(),
        bucket_endpoint_url: mount.endpoint_url.clone().unwrap_or_else(|| {
            mount
                .region
                .as_ref()
                .map(|region| format!("https://s3.{region}.amazonaws.com"))
                .unwrap_or_else(|| "https://s3.amazonaws.com".to_owned())
        }),
        provider: HostedBucketProvider::S3,
        key_prefix: normalize_cloudflare_prefix(mount.prefix.as_deref()),
        credentials: build_access_key_credentials(
            mount.access_key_id.as_deref(),
            mount.secret_access_key.as_deref(),
        ),
        read_only: mount.read_only,
    })
}

fn build_cloudflare_r2_payload(mount: &HostedR2Mount) -> CloudflareBucketMountPayload {
    CloudflareBucketMountPayload {
        bucket_name: mount.bucket.clone(),
        bucket_endpoint_url: r2_endpoint(&mount.account_id, mount.custom_domain.as_deref()),
        provider: HostedBucketProvider::R2,
        key_prefix: None,
        credentials: build_access_key_credentials(
            mount.access_key_id.as_deref(),
            mount.secret_access_key.as_deref(),
        ),
        read_only: mount.read_only,
    }
}

fn build_cloudflare_gcs_payload(
    mount: &HostedGcsMount,
) -> HostedSandboxResult<CloudflareBucketMountPayload> {
    let using_hmac = mount.access_id.is_some() && mount.secret_access_key.is_some();
    if !using_hmac {
        return Err(HostedSandboxError::new(
            "gcs cloudflare bucket mounts require access_id and secret_access_key",
        ));
    }
    if mount.service_account_credentials.is_some() || mount.access_token.is_some() {
        return Err(HostedSandboxError::new(
            "gcs cloudflare bucket mounts require access_id and secret_access_key",
        ));
    }
    Ok(CloudflareBucketMountPayload {
        bucket_name: mount.bucket.clone(),
        bucket_endpoint_url: mount
            .endpoint_url
            .clone()
            .unwrap_or_else(|| "https://storage.googleapis.com".to_owned()),
        provider: HostedBucketProvider::Gcs,
        key_prefix: normalize_cloudflare_prefix(mount.prefix.as_deref()),
        credentials: build_access_key_credentials(
            mount.access_id.as_deref(),
            mount.secret_access_key.as_deref(),
        ),
        read_only: mount.read_only,
    })
}

fn validate_access_key_pair(
    label: &str,
    access_key_id: Option<&str>,
    secret_access_key: Option<&str>,
) -> HostedSandboxResult<()> {
    if access_key_id.is_some() != secret_access_key.is_some() {
        return Err(HostedSandboxError::new(format!(
            "{label} require both access_key_id and secret_access_key when either is provided"
        )));
    }
    Ok(())
}

fn build_access_key_credentials(
    access_key_id: Option<&str>,
    secret_access_key: Option<&str>,
) -> Option<HostedAccessKeyCredentials> {
    match (access_key_id, secret_access_key) {
        (Some(access_key_id), Some(secret_access_key)) => Some(HostedAccessKeyCredentials {
            access_key_id: access_key_id.to_owned(),
            secret_access_key: secret_access_key.to_owned(),
        }),
        _ => None,
    }
}

fn join_remote_path(bucket: &str, prefix: Option<&str>) -> String {
    match prefix
        .map(|prefix| prefix.trim_matches('/'))
        .filter(|prefix| !prefix.is_empty())
    {
        Some(prefix) => format!("{bucket}/{prefix}"),
        None => bucket.to_owned(),
    }
}

fn r2_endpoint(account_id: &str, custom_domain: Option<&str>) -> String {
    custom_domain
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("https://{account_id}.r2.cloudflarestorage.com"))
}

fn normalize_cloudflare_prefix(prefix: Option<&str>) -> Option<String> {
    match prefix {
        None => None,
        Some(prefix) => {
            let trimmed = prefix.trim_matches('/');
            if trimmed.is_empty() {
                Some("/".to_owned())
            } else {
                Some(format!("/{trimmed}/"))
            }
        }
    }
}

fn validate_modal_secret_fields(
    secret_name: Option<&str>,
    secret_environment_name: Option<&str>,
    mount_type: &str,
) -> HostedSandboxResult<()> {
    if matches!(secret_name, Some("")) {
        return Err(HostedSandboxError::new(format!(
            "modal cloud bucket secret_name must be a non-empty string for {mount_type}"
        )));
    }
    if matches!(secret_environment_name, Some("")) {
        return Err(HostedSandboxError::new(format!(
            "modal cloud bucket secret_environment_name must be a non-empty string for {mount_type}"
        )));
    }
    if secret_environment_name.is_some() && secret_name.is_none() {
        return Err(HostedSandboxError::new(
            "modal cloud bucket secret_environment_name requires secret_name to also be set",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HostedSandboxClientOptionsBase {
    pub workspace_root: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub token: Option<String>,
    pub client_timeout_s: Option<u64>,
    pub exposed_ports: Vec<u16>,
    pub interactive_pty: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HostedSandboxSessionStateBase {
    pub session_id: String,
    pub workspace_root: String,
    pub base_url: Option<String>,
    pub exposed_ports: Vec<u16>,
    pub interactive_pty: bool,
    pub start_state_preserved: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg(any(
    feature = "e2b",
    feature = "modal",
    feature = "daytona",
    feature = "blaxel",
    feature = "cloudflare",
    feature = "runloop",
    feature = "vercel"
))]
struct HostedResolvedAuth {
    value: String,
    source: &'static str,
}

#[cfg(any(
    feature = "e2b",
    feature = "modal",
    feature = "daytona",
    feature = "blaxel",
    feature = "cloudflare",
    feature = "runloop",
    feature = "vercel"
))]
fn next_hosted_session_id(provider_name: &str) -> String {
    let id = NEXT_HOSTED_SESSION_ID.fetch_add(1, Ordering::Relaxed);
    format!("{provider_name}-session-{id}")
}

#[cfg(any(
    feature = "e2b",
    feature = "modal",
    feature = "daytona",
    feature = "blaxel",
    feature = "cloudflare",
    feature = "runloop",
    feature = "vercel"
))]
fn normalize_workspace_root(
    provider_name: &str,
    policy: WorkspaceRootPolicy,
    requested_root: Option<&str>,
) -> HostedSandboxResult<String> {
    match policy {
        WorkspaceRootPolicy::Strict(required_root) => match requested_root {
            Some(root) if root != required_root => Err(HostedSandboxError::new(format!(
                "{provider_name} sandboxes require workspace_root={required_root:?}, got {root:?}"
            ))),
            Some(root) => Ok(root.to_owned()),
            None => Ok(required_root.to_owned()),
        },
        WorkspaceRootPolicy::Defaulted(default_root) => {
            Ok(requested_root.unwrap_or(default_root).to_owned())
        }
    }
}

#[cfg(any(
    feature = "e2b",
    feature = "modal",
    feature = "daytona",
    feature = "blaxel",
    feature = "cloudflare",
    feature = "runloop",
    feature = "vercel"
))]
fn resolve_auth(
    provider_name: &str,
    auth_kind: HostedAuthKind,
    auth_env_var: &str,
    api_key: Option<&str>,
    token: Option<&str>,
) -> HostedSandboxResult<HostedResolvedAuth> {
    let explicit = match auth_kind {
        HostedAuthKind::ApiKey => api_key.filter(|value| !value.is_empty()),
        HostedAuthKind::Token => token.filter(|value| !value.is_empty()),
    };
    if let Some(value) = explicit {
        return Ok(HostedResolvedAuth {
            value: value.to_owned(),
            source: "explicit",
        });
    }

    if let Ok(value) = std::env::var(auth_env_var) {
        if !value.is_empty() {
            return Ok(HostedResolvedAuth {
                value,
                source: "env",
            });
        }
    }

    let auth_label = match auth_kind {
        HostedAuthKind::ApiKey => "api_key",
        HostedAuthKind::Token => "token",
    };
    Err(HostedSandboxError::new(format!(
        "{provider_name} sandboxes require {auth_label} or {auth_env_var}"
    )))
}

#[cfg(any(
    feature = "e2b",
    feature = "modal",
    feature = "daytona",
    feature = "blaxel",
    feature = "cloudflare",
    feature = "runloop",
    feature = "vercel"
))]
fn validate_capabilities(
    provider_name: &str,
    supports_exposed_ports: bool,
    supports_pty: bool,
    exposed_ports: &[u16],
    interactive_pty: bool,
) -> HostedSandboxResult<()> {
    if !supports_exposed_ports && !exposed_ports.is_empty() {
        return Err(HostedSandboxError::new(format!(
            "{provider_name} sandboxes do not support exposed ports"
        )));
    }
    if interactive_pty && !supports_pty {
        return Err(HostedSandboxError::new(format!(
            "{provider_name} sandboxes do not support interactive PTY sessions"
        )));
    }
    Ok(())
}

macro_rules! define_hosted_sandbox_provider {
    ($mod_name:ident, $client:ident, $options:ident, $session:ident, $state:ident, $spec:expr) => {
        pub mod $mod_name {
            use super::*;

            const SPEC: HostedProviderSpec = $spec;

            #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
            pub struct $options {
                pub workspace_root: Option<String>,
                pub base_url: Option<String>,
                pub api_key: Option<String>,
                pub token: Option<String>,
                pub client_timeout_s: Option<u64>,
                pub exposed_ports: Vec<u16>,
                pub interactive_pty: bool,
            }

            #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
            pub struct $state {
                pub session_id: String,
                pub workspace_root: String,
                pub base_url: Option<String>,
                pub exposed_ports: Vec<u16>,
                pub interactive_pty: bool,
                pub start_state_preserved: bool,
            }

            impl Default for $state {
                fn default() -> Self {
                    Self {
                        session_id: String::new(),
                        workspace_root: normalize_workspace_root(
                            SPEC.provider_name,
                            SPEC.workspace_root_policy,
                            None,
                        )
                        .expect("default workspace root should be valid"),
                        base_url: None,
                        exposed_ports: Vec::new(),
                        interactive_pty: false,
                        start_state_preserved: false,
                    }
                }
            }

            #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
            pub struct $client {
                pub options: $options,
            }

            impl $client {
                pub fn new(options: $options) -> Self {
                    Self { options }
                }

                pub fn options(&self) -> &$options {
                    &self.options
                }

                pub fn create(&self) -> HostedSandboxResult<$session> {
                    let auth = resolve_auth(
                        SPEC.provider_name,
                        SPEC.auth_kind,
                        SPEC.auth_env_var,
                        self.options.api_key.as_deref(),
                        self.options.token.as_deref(),
                    )?;
                    validate_capabilities(
                        SPEC.provider_name,
                        SPEC.supports_exposed_ports,
                        SPEC.supports_pty,
                        &self.options.exposed_ports,
                        self.options.interactive_pty,
                    )?;

                    let workspace_root = normalize_workspace_root(
                        SPEC.provider_name,
                        SPEC.workspace_root_policy,
                        self.options.workspace_root.as_deref(),
                    )?;

                    Ok($session {
                        state: $state {
                            session_id: next_hosted_session_id(SPEC.provider_name),
                            workspace_root,
                            base_url: self.options.base_url.clone(),
                            exposed_ports: self.options.exposed_ports.clone(),
                            interactive_pty: self.options.interactive_pty,
                            start_state_preserved: false,
                        },
                        resolved_auth: auth,
                    })
                }

                pub fn resume(&self, state: $state) -> HostedSandboxResult<$session> {
                    let auth = resolve_auth(
                        SPEC.provider_name,
                        SPEC.auth_kind,
                        SPEC.auth_env_var,
                        self.options.api_key.as_deref(),
                        self.options.token.as_deref(),
                    )?;
                    validate_capabilities(
                        SPEC.provider_name,
                        SPEC.supports_exposed_ports,
                        SPEC.supports_pty,
                        &state.exposed_ports,
                        state.interactive_pty,
                    )?;

                    let workspace_root = normalize_workspace_root(
                        SPEC.provider_name,
                        SPEC.workspace_root_policy,
                        Some(&state.workspace_root),
                    )?;

                    Ok($session {
                        state: $state {
                            session_id: state.session_id,
                            workspace_root,
                            base_url: state.base_url,
                            exposed_ports: state.exposed_ports,
                            interactive_pty: state.interactive_pty,
                            start_state_preserved: true,
                        },
                        resolved_auth: auth,
                    })
                }

                pub fn serialize_session_state(
                    &self,
                    state: &$state,
                ) -> HostedSandboxResult<serde_json::Value> {
                    serde_json::to_value(state)
                        .map_err(|error| HostedSandboxError::new(error.to_string()))
                }

                pub fn deserialize_session_state(
                    &self,
                    payload: serde_json::Value,
                ) -> HostedSandboxResult<$state> {
                    serde_json::from_value(payload)
                        .map_err(|error| HostedSandboxError::new(error.to_string()))
                }
            }

            #[derive(Clone, Debug, PartialEq, Eq)]
            pub struct $session {
                pub state: $state,
                resolved_auth: HostedResolvedAuth,
            }

            impl Default for $session {
                fn default() -> Self {
                    Self {
                        state: $state::default(),
                        resolved_auth: HostedResolvedAuth {
                            value: String::new(),
                            source: "explicit",
                        },
                    }
                }
            }

            impl $session {
                pub fn new(state: $state) -> Self {
                    Self {
                        state,
                        resolved_auth: HostedResolvedAuth {
                            value: String::new(),
                            source: "explicit",
                        },
                    }
                }

                pub fn state(&self) -> &$state {
                    &self.state
                }

                pub fn resolved_auth_source(&self) -> &'static str {
                    self.resolved_auth.source
                }

                pub fn resolved_auth_value(&self) -> &str {
                    &self.resolved_auth.value
                }

                pub fn supports_pty(&self) -> bool {
                    SPEC.supports_pty
                }
            }
        }

        pub use $mod_name::{$client, $options, $session, $state};
    };
}

#[cfg(feature = "e2b")]
define_hosted_sandbox_provider!(
    e2b,
    E2BSandboxClient,
    E2BSandboxClientOptions,
    E2BSandboxSession,
    E2BSandboxSessionState,
    HostedProviderSpec {
        provider_name: "e2b",
        auth_kind: HostedAuthKind::ApiKey,
        auth_env_var: "E2B_API_KEY",
        workspace_root_policy: WorkspaceRootPolicy::Defaulted(DEFAULT_E2B_WORKSPACE_ROOT),
        supports_exposed_ports: true,
        supports_pty: true,
    }
);
#[cfg(feature = "modal")]
define_hosted_sandbox_provider!(
    modal,
    ModalSandboxClient,
    ModalSandboxClientOptions,
    ModalSandboxSession,
    ModalSandboxSessionState,
    HostedProviderSpec {
        provider_name: "modal",
        auth_kind: HostedAuthKind::Token,
        auth_env_var: "MODAL_TOKEN_ID",
        workspace_root_policy: WorkspaceRootPolicy::Defaulted(DEFAULT_MODAL_WORKSPACE_ROOT),
        supports_exposed_ports: true,
        supports_pty: false,
    }
);
#[cfg(feature = "daytona")]
define_hosted_sandbox_provider!(
    daytona,
    DaytonaSandboxClient,
    DaytonaSandboxClientOptions,
    DaytonaSandboxSession,
    DaytonaSandboxSessionState,
    HostedProviderSpec {
        provider_name: "daytona",
        auth_kind: HostedAuthKind::ApiKey,
        auth_env_var: "DAYTONA_API_KEY",
        workspace_root_policy: WorkspaceRootPolicy::Defaulted(DEFAULT_DAYTONA_WORKSPACE_ROOT),
        supports_exposed_ports: true,
        supports_pty: true,
    }
);
#[cfg(feature = "blaxel")]
define_hosted_sandbox_provider!(
    blaxel,
    BlaxelSandboxClient,
    BlaxelSandboxClientOptions,
    BlaxelSandboxSession,
    BlaxelSandboxSessionState,
    HostedProviderSpec {
        provider_name: "blaxel",
        auth_kind: HostedAuthKind::Token,
        auth_env_var: "BL_API_KEY",
        workspace_root_policy: WorkspaceRootPolicy::Defaulted(DEFAULT_BLAXEL_WORKSPACE_ROOT),
        supports_exposed_ports: true,
        supports_pty: true,
    }
);
#[cfg(feature = "cloudflare")]
define_hosted_sandbox_provider!(
    cloudflare,
    CloudflareSandboxClient,
    CloudflareSandboxClientOptions,
    CloudflareSandboxSession,
    CloudflareSandboxSessionState,
    HostedProviderSpec {
        provider_name: "cloudflare",
        auth_kind: HostedAuthKind::ApiKey,
        auth_env_var: "CLOUDFLARE_SANDBOX_API_KEY",
        workspace_root_policy: WorkspaceRootPolicy::Strict(DEFAULT_CLOUDFLARE_WORKSPACE_ROOT),
        supports_exposed_ports: true,
        supports_pty: true,
    }
);
#[cfg(feature = "runloop")]
define_hosted_sandbox_provider!(
    runloop,
    RunloopSandboxClient,
    RunloopSandboxClientOptions,
    RunloopSandboxSession,
    RunloopSandboxSessionState,
    HostedProviderSpec {
        provider_name: "runloop",
        auth_kind: HostedAuthKind::ApiKey,
        auth_env_var: "RUNLOOP_API_KEY",
        workspace_root_policy: WorkspaceRootPolicy::Defaulted(DEFAULT_RUNLOOP_WORKSPACE_ROOT),
        supports_exposed_ports: true,
        supports_pty: false,
    }
);
#[cfg(feature = "vercel")]
define_hosted_sandbox_provider!(
    vercel,
    VercelSandboxClient,
    VercelSandboxClientOptions,
    VercelSandboxSession,
    VercelSandboxSessionState,
    HostedProviderSpec {
        provider_name: "vercel",
        auth_kind: HostedAuthKind::Token,
        auth_env_var: "VERCEL_TOKEN",
        workspace_root_policy: WorkspaceRootPolicy::Defaulted(DEFAULT_VERCEL_WORKSPACE_ROOT),
        supports_exposed_ports: true,
        supports_pty: false,
    }
);
