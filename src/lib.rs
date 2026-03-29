//! # kube-eks-config
//!
//! Helpers for building a [`kube_client::Client`] (or [`kube_client::Config`])
//! directly from an [Amazon EKS](https://aws.amazon.com/eks/) cluster, without
//! manually managing a kubeconfig file on disk.
//!
//! ## How it works
//!
//! The crate calls the AWS EKS `DescribeCluster` API to retrieve the cluster's
//! HTTPS endpoint and certificate-authority data, then converts those values
//! into the configuration structs used by `kube_client`.  Authentication
//! (bearer tokens) is intentionally omitted: EKS uses short-lived tokens
//! obtained via `aws eks get-token`, IRSA, or EKS Pod Identity — none of which
//! belong in a static config.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use kube_eks_config::{TryEksClusterExt, default_aws_client};
//!
//! #[tokio::main]
//! async fn main() -> kube_client::Result<()> {
//!     // Credentials are loaded from the environment (see [`default_aws_client`])
//!     let aws = default_aws_client().await;
//!
//!     // One call produces a ready-to-use Kubernetes client
//!     let client = aws.try_eks_kube_client("my-cluster").await?;
//!     let _ = client;
//!     Ok(())
//! }
//! ```
//!
//! ## AWS credentials
//!
//! [`default_aws_client`] resolves credentials via the standard AWS provider
//! chain (highest priority first):
//!
//! 1. Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, …)
//! 2. AWS shared credentials / config files (`~/.aws/credentials`)
//! 3. Web identity / IRSA (`AWS_WEB_IDENTITY_TOKEN_FILE` + `AWS_ROLE_ARN`)
//! 4. Amazon EC2 / ECS instance metadata (IMDSv2)
//!
//! Any custom [`aws_sdk_eks::Client`] can also be used directly with the
//! [`TryEksClusterExt`] methods.
//!
//! ## Traits at a glance
//!
//! | Trait | Input | Output |
//! |---|---|---|
//! | [`TryEksClusterExt`] | `eks::Client` + cluster name | cluster / config / client |
//! | [`ToKubeConfig`] | `eks::types::Cluster` | `kube_client::Config` |
//! | [`IntoKubeconfig`] | `eks::types::Cluster` | `kube_client::config::Kubeconfig` |

use aws_sdk_eks as eks;
use kube_client::Error as KubeError;
use kube_client::config as kubeconfig;

/// Extension trait that adds EKS-aware helpers to [`aws_sdk_eks::Client`].
///
/// The three methods form a convenience ladder — use the one that returns
/// exactly what you need without paying for extra AWS API calls:
///
/// | Method | Returns |
/// |---|---|
/// | [`try_eks_cluster`](Self::try_eks_cluster) | Raw [`eks::types::Cluster`] from the AWS API |
/// | [`try_eks_kube_config`](Self::try_eks_kube_config) | [`kube_client::Config`] ready for `Client::try_from` |
/// | [`try_eks_kube_client`](Self::try_eks_kube_client) | Fully initialised [`kube_client::Client`] |
///
/// Most callers only need [`try_eks_kube_client`](Self::try_eks_kube_client).
/// The lower-level methods are exposed so that intermediate values can be
/// inspected or reused without making additional AWS API calls.
///
/// # Example
///
/// ```rust,no_run
/// use kube_eks_config::{TryEksClusterExt, default_aws_client};
///
/// # #[tokio::main]
/// # async fn main() -> kube_client::Result<()> {
/// let aws = default_aws_client().await;
/// let client = aws.try_eks_kube_client("my-cluster").await?;
/// let _ = client;
/// # Ok(())
/// # }
/// ```
// `async fn` in traits is stable since Rust 1.75 but still triggers a lint;
// `#[expect]` silences it and documents the intent.
#[expect(async_fn_in_trait)]
pub trait TryEksClusterExt {
    /// Fetches the EKS cluster descriptor from the AWS API.
    ///
    /// Returns the raw [`eks::types::Cluster`] struct, which contains the
    /// HTTPS endpoint URL, certificate-authority data, cluster status,
    /// Kubernetes version, tags, and other metadata.
    ///
    /// # Errors
    ///
    /// - [`eks::Error::NotFoundException`] if no cluster with the given name
    ///   exists in the caller's AWS account and region.
    /// - Any other [`eks::Error`] variant on API or network failures.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use kube_eks_config::{TryEksClusterExt, default_aws_client};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), aws_sdk_eks::Error> {
    /// let aws = default_aws_client().await;
    /// let cluster = aws.try_eks_cluster("my-cluster").await?;
    /// println!("Kubernetes version: {:?}", cluster.version);
    /// # Ok(())
    /// # }
    /// ```
    async fn try_eks_cluster(
        &self,
        cluster: impl Into<String>,
    ) -> Result<eks::types::Cluster, eks::Error>;

    /// Builds a [`kube_client::Config`] for the named EKS cluster.
    ///
    /// This is a provided method: it calls [`try_eks_cluster`](Self::try_eks_cluster)
    /// and converts the result via [`ToKubeConfig::into_kube_config`].
    ///
    /// The resulting `Config` holds the cluster's HTTPS endpoint and
    /// certificate-authority data but does **not** contain authentication
    /// credentials — EKS authentication is handled via short-lived tokens
    /// (IRSA, EKS Pod Identity, `aws eks get-token`).
    ///
    /// # Errors
    ///
    /// - [`kube_client::Error::Service`] wrapping an [`eks::Error`] on AWS failures.
    /// - [`kube_client::Error::InferKubeconfig`] if the endpoint URL is absent
    ///   or cannot be parsed.
    async fn try_eks_kube_config(
        &self,
        cluster: impl Into<String>,
    ) -> Result<kube_client::Config, KubeError> {
        self.try_eks_cluster(cluster)
            .await
            .map_err(|err| KubeError::Service(Box::new(err)))?
            .into_kube_config()
            .map_err(KubeError::InferKubeconfig)
    }

    /// Creates a [`kube_client::Client`] connected to the named EKS cluster.
    ///
    /// This is the primary convenience method. It combines
    /// [`try_eks_kube_config`](Self::try_eks_kube_config) and
    /// [`kube_client::Client::try_from`] into a single call.
    ///
    /// # Errors
    ///
    /// Propagates all errors from
    /// [`try_eks_kube_config`](Self::try_eks_kube_config) plus any TLS or HTTP
    /// client initialisation errors from `kube_client`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use kube_eks_config::{TryEksClusterExt, default_aws_client};
    ///
    /// #[tokio::main]
    /// async fn main() -> kube_client::Result<()> {
    ///     let aws = default_aws_client().await;
    ///     let client = aws.try_eks_kube_client("my-cluster").await?;
    ///     let _ = client; // use with kube::Api for Kubernetes operations
    ///     Ok(())
    /// }
    /// ```
    async fn try_eks_kube_client(
        &self,
        cluster: impl Into<String>,
    ) -> Result<kube_client::Client, KubeError> {
        let config = self.try_eks_kube_config(cluster).await?;
        kube_client::Client::try_from(config)
    }
}

impl TryEksClusterExt for eks::Client {
    async fn try_eks_cluster(
        &self,
        cluster: impl Into<String>,
    ) -> Result<eks::types::Cluster, eks::Error> {
        let cluster = cluster.into();
        self.describe_cluster()
            .name(&cluster)
            .send()
            .await?
            .cluster
            .ok_or_else(|| cluster_not_found(&cluster))
    }
}

/// Converts an [`eks::types::Cluster`] into a [`kube_client::Config`].
///
/// This lower-level conversion is used internally by
/// [`TryEksClusterExt::try_eks_kube_config`]. It is also useful when you
/// already hold a `Cluster` value and want a runtime `Config` without making
/// an additional AWS API call.
///
/// See also [`IntoKubeconfig`] for converting to the serialisable
/// [`kube_client::config::Kubeconfig`] format (the equivalent of a kubeconfig
/// YAML file).
pub trait ToKubeConfig {
    /// Converts `self` into a [`kube_client::Config`].
    ///
    /// Extracts the cluster's `endpoint` (required) and
    /// `certificate_authority.data` (optional). No authentication credentials
    /// are included — EKS uses short-lived bearer tokens.
    ///
    /// # Errors
    ///
    /// - [`kube_client::config::KubeconfigError::MissingClusterUrl`] if the
    ///   cluster's `endpoint` field is `None`.
    /// - [`kube_client::config::KubeconfigError::ParseClusterUrl`] if the
    ///   endpoint string is not a valid URL.
    fn into_kube_config(self) -> Result<kubeconfig::Config, kubeconfig::KubeconfigError>;
}

impl ToKubeConfig for eks::types::Cluster {
    fn into_kube_config(self) -> Result<kubeconfig::Config, kubeconfig::KubeconfigError> {
        let client_certificate_data = self.certificate_authority.and_then(|cert| cert.data);
        let auth_info = kubeconfig::AuthInfo {
            client_certificate_data,
            ..kubeconfig::AuthInfo::default()
        };
        let cluster_url = self
            .endpoint
            .ok_or(kubeconfig::KubeconfigError::MissingClusterUrl)?
            .try_into()
            .map_err(kubeconfig::KubeconfigError::ParseClusterUrl)?;
        let config = kubeconfig::Config {
            auth_info,
            ..kubeconfig::Config::new(cluster_url)
        };

        Ok(config)
    }
}

/// Converts an [`eks::types::Cluster`] into a
/// [`kube_client::config::Kubeconfig`].
///
/// Unlike [`ToKubeConfig`] (which produces a `kube_client::Config` runtime
/// struct), this trait produces the serialisable
/// [`kube_client::config::Kubeconfig`] structure — the in-memory equivalent of
/// a `~/.kube/config` file — with named cluster, context, and
/// `current-context` entries.
///
/// This is useful when you need to:
///
/// - Serialise the kubeconfig to YAML and write it to disk.
/// - Merge the EKS cluster entry into an existing kubeconfig.
/// - Pass a structured kubeconfig to tooling that expects the full format.
///
/// # Authentication
///
/// The produced `Kubeconfig` contains **no `auth_infos` entries**. EKS
/// authentication relies on short-lived bearer tokens obtained outside of
/// static kubeconfig credentials (e.g. via IRSA, EKS Pod Identity, or
/// `aws eks get-token`). Callers are responsible for supplying an
/// `exec`-based `AuthInfo` if they need a fully self-contained kubeconfig.
///
/// See also [`ToKubeConfig`] for a direct runtime `Config` conversion.
pub trait IntoKubeconfig {
    /// Converts `self` into a [`kube_client::config::Kubeconfig`].
    ///
    /// The cluster name (falls back to `"eks-cluster"` if absent) is used as
    /// the `clusters[0].name`, `contexts[0].name`,
    /// `contexts[0].context.cluster`, and `current-context`.
    ///
    /// # Errors
    ///
    /// Currently infallible in practice, but returns a `Result` for forward
    /// compatibility.
    fn into_kubeconfig(self) -> Result<kubeconfig::Kubeconfig, kubeconfig::KubeconfigError>;
}

impl IntoKubeconfig for eks::types::Cluster {
    fn into_kubeconfig(self) -> Result<kubeconfig::Kubeconfig, kubeconfig::KubeconfigError> {
        let eks::types::Cluster {
            name,
            endpoint,
            certificate_authority,
            // arn,
            // created_at,
            // version,
            // role_arn,
            // resources_vpc_config,
            // kubernetes_network_config,
            // logging,
            // identity,
            // status,
            // client_request_token,
            // platform_version,
            // tags,
            // encryption_config,
            // connector_config,
            // id,
            // health,
            // outpost_config,
            // access_config,
            // upgrade_policy,
            // zonal_shift_config,
            // remote_network_config,
            // compute_config,
            // storage_config,
            // deletion_protection,
            ..
        } = self;
        let name = name.unwrap_or_else(|| "eks-cluster".to_string());
        let certificate_authority_data = certificate_authority.and_then(|cert| cert.data);

        let cluster = kubeconfig::Cluster {
            server: endpoint,
            insecure_skip_tls_verify: None,
            certificate_authority: None,
            certificate_authority_data,
            proxy_url: None,
            disable_compression: None,
            tls_server_name: None,
            extensions: None,
        };

        let named_cluster = kubeconfig::NamedCluster {
            name: name.clone(),
            cluster: Some(cluster),
        };

        let context = kubeconfig::Context {
            cluster: name.clone(),
            user: None,
            namespace: None,
            extensions: None,
        };

        let named_context = kubeconfig::NamedContext {
            name: name.clone(),
            context: Some(context),
        };

        let config = kubeconfig::Kubeconfig {
            clusters: vec![named_cluster],
            contexts: vec![named_context],
            current_context: Some(name),
            // auth_infos: vec![],
            ..kubeconfig::Kubeconfig::default()
        };

        Ok(config)
    }
}

// Constructs a `NotFoundException` carrying a human-readable message that
// names the missing cluster. Used when `DescribeCluster` returns `None`.
fn cluster_not_found(cluster: &str) -> eks::Error {
    let exception = eks::types::error::NotFoundException::builder()
        .message(format!("EKS cluster {cluster} not found"))
        .build();
    eks::Error::NotFoundException(exception)
}

/// Creates an [`aws_sdk_eks::Client`] from the default AWS credential chain.
///
/// Credentials and the AWS region are resolved in the following order
/// (highest priority first):
///
/// 1. **Environment variables** — `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`,
///    `AWS_SESSION_TOKEN`, `AWS_REGION` / `AWS_DEFAULT_REGION`.
/// 2. **AWS shared files** — `~/.aws/credentials` and `~/.aws/config`.
/// 3. **Web identity / IRSA** — `AWS_WEB_IDENTITY_TOKEN_FILE` + `AWS_ROLE_ARN`
///    (used in Kubernetes pods with IAM Roles for Service Accounts).
/// 4. **Instance metadata** — EC2 instance profile or ECS task role via the
///    IMDSv2 endpoint.
///
/// This is a thin convenience wrapper around [`aws_config::load_from_env`].
/// For fine-grained control over credentials, region, or endpoint
/// configuration, construct an [`aws_sdk_eks::Client`] directly and use
/// [`TryEksClusterExt`] on it.
///
/// # Example
///
/// ```rust,no_run
/// use kube_eks_config::{TryEksClusterExt, default_aws_client};
///
/// # #[tokio::main]
/// # async fn main() -> kube_client::Result<()> {
/// let aws = default_aws_client().await;
/// let client = aws.try_eks_kube_client("my-cluster").await?;
/// let _ = client;
/// # Ok(())
/// # }
/// ```
pub async fn default_aws_client() -> eks::Client {
    let config = aws_config::load_from_env().await;
    eks::Client::new(&config)
}

#[cfg(test)]
mod tests {
    use super::{IntoKubeconfig, ToKubeConfig};
    use aws_sdk_eks as eks;
    use kube_client::config as kubeconfig;

    /// Constructs an `eks::types::Cluster` from optional parts without hitting AWS.
    fn make_cluster(
        name: Option<&str>,
        endpoint: Option<&str>,
        cert_data: Option<&str>,
    ) -> eks::types::Cluster {
        let mut builder = eks::types::Cluster::builder();
        if let Some(n) = name {
            builder = builder.name(n);
        }
        if let Some(e) = endpoint {
            builder = builder.endpoint(e);
        }
        if let Some(d) = cert_data {
            builder =
                builder.certificate_authority(eks::types::Certificate::builder().data(d).build());
        }
        builder.build()
    }

    #[test]
    fn cluster_not_found_error_contains_name() {
        let err = super::cluster_not_found("my-cluster");
        let eks::Error::NotFoundException(ref ex) = err else {
            panic!("expected NotFoundException, got {err:?}");
        };
        assert!(
            ex.message().unwrap_or("").contains("my-cluster"),
            "error message should contain the cluster name"
        );
    }

    #[test]
    fn to_kube_config_extracts_endpoint_and_cert() {
        let cluster = make_cluster(
            Some("test"),
            Some("https://abc123.gr7.us-east-1.eks.amazonaws.com"),
            Some("base64certdata=="),
        );
        let config = cluster.into_kube_config().expect("should build config");
        assert_eq!(
            config.cluster_url.host(),
            Some("abc123.gr7.us-east-1.eks.amazonaws.com")
        );
        assert_eq!(
            config.auth_info.client_certificate_data.as_deref(),
            Some("base64certdata==")
        );
    }

    #[test]
    fn to_kube_config_missing_endpoint_returns_error() {
        let cluster = make_cluster(Some("test"), None, None);
        let err = cluster.into_kube_config().unwrap_err();
        assert!(
            matches!(err, kubeconfig::KubeconfigError::MissingClusterUrl),
            "expected MissingClusterUrl, got {err:?}"
        );
    }

    #[test]
    fn into_kubeconfig_uses_cluster_name() {
        let cluster = make_cluster(Some("my-cluster"), Some("https://example.k8s.io"), None);
        let kc = cluster.into_kubeconfig().expect("should build kubeconfig");
        assert_eq!(kc.current_context.as_deref(), Some("my-cluster"));
        assert_eq!(kc.clusters[0].name, "my-cluster");
        assert_eq!(kc.contexts[0].name, "my-cluster");
        assert_eq!(
            kc.contexts[0].context.as_ref().map(|c| c.cluster.as_str()),
            Some("my-cluster")
        );
    }

    #[test]
    fn into_kubeconfig_falls_back_to_eks_cluster_name() {
        let cluster = make_cluster(None, Some("https://example.k8s.io"), None);
        let kc = cluster.into_kubeconfig().expect("should build kubeconfig");
        assert_eq!(kc.current_context.as_deref(), Some("eks-cluster"));
        assert_eq!(kc.clusters[0].name, "eks-cluster");
    }

    #[test]
    fn into_kubeconfig_propagates_cert_authority_data() {
        let cluster = make_cluster(Some("test"), None, Some("dGVzdA=="));
        let kc = cluster.into_kubeconfig().expect("should build kubeconfig");
        let cert = kc.clusters[0]
            .cluster
            .as_ref()
            .and_then(|c| c.certificate_authority_data.as_deref());
        assert_eq!(cert, Some("dGVzdA=="));
    }
}
