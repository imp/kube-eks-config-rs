//! # kube-eks-config
//!
//! A helper crate for creating [`kube_client::Config`] and [`kube_client::Client`] for AWS EKS
//! clusters.
//!
//! ## Overview
//!
//! This crate bridges the [AWS SDK for Rust](https://github.com/awslabs/aws-sdk-rust) and the
//! [`kube`](https://docs.rs/kube) Kubernetes client library.  Given the name of an EKS cluster it
//! calls the AWS API to retrieve the cluster's API-server endpoint and certificate-authority data,
//! then constructs a fully configured [`kube_client::Client`] ready to make Kubernetes API calls.
//!
//! ## Prerequisites
//!
//! AWS credentials must be available in the environment.  [`default_aws_client`] (and the
//! underlying [`aws_config::load_from_env`]) honours the standard AWS credential resolution chain:
//!
//! * Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN`, …)
//! * Shared credentials file (`~/.aws/credentials`) and config file (`~/.aws/config`)
//! * IAM roles attached to EC2 instances, ECS tasks, or Lambda functions
//! * AWS IAM Identity Center (SSO) profiles
//!
//! The IAM principal must have permission to call `eks:DescribeCluster` on the target cluster.
//!
//! ## Usage
//!
//! ### Quick start — one-liner client
//!
//! ```no_run
//! use kube_eks_config::{TryEksClusterExt, default_aws_client};
//!
//! #[tokio::main]
//! async fn main() -> kube_client::Result<()> {
//!     let aws = default_aws_client().await;
//!     let client = aws.try_eks_kube_client("my-cluster").await?;
//!     // `client` is ready to make Kubernetes API calls against "my-cluster"
//!     println!("Connected to EKS cluster");
//!     Ok(())
//! }
//! ```
//!
//! ### Inspect the config before creating a client
//!
//! ```no_run
//! use kube_eks_config::{TryEksClusterExt, default_aws_client};
//!
//! #[tokio::main]
//! async fn main() -> kube_client::Result<()> {
//!     let aws = default_aws_client().await;
//!     let config = aws.try_eks_kube_config("my-cluster").await?;
//!     println!("Cluster endpoint: {}", config.cluster_url);
//!     let client = kube_client::Client::try_from(config)?;
//!     Ok(())
//! }
//! ```
//!
//! ### Using a custom AWS configuration (explicit region or profile)
//!
//! ```no_run
//! use kube_eks_config::TryEksClusterExt;
//!
//! #[tokio::main]
//! async fn main() -> kube_client::Result<()> {
//!     let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
//!         .region(aws_config::meta::region::RegionProviderChain::default_provider()
//!             .or_else("us-east-1"))
//!         .load()
//!         .await;
//!     let aws = aws_sdk_eks::Client::new(&aws_config);
//!     let client = aws.try_eks_kube_client("my-cluster").await?;
//!     Ok(())
//! }
//! ```
//!
//! ## How it works
//!
//! 1. [`TryEksClusterExt::try_eks_cluster`] calls [`aws_sdk_eks::Client::describe_cluster`] to
//!    fetch the cluster metadata from the EKS API.
//! 2. [`ToKubeConfig::into_kube_config`] extracts the API-server endpoint URL and the
//!    base64-encoded certificate-authority data from the cluster metadata and builds a
//!    [`kube_client::Config`].
//! 3. [`TryEksClusterExt::try_eks_kube_client`] wraps the config in a
//!    [`kube_client::Client`] that is ready to use.

use aws_sdk_eks as eks;
use kube_client::Error as KubeError;
use kube_client::config as kubeconfig;

/// Extension trait that adds EKS-aware helper methods to [`aws_sdk_eks::Client`].
///
/// All methods accept the EKS cluster *name* (not the ARN) and return typed errors so callers
/// can handle AWS and Kubernetes failures uniformly.
///
/// # Examples
///
/// ```no_run
/// use kube_eks_config::{TryEksClusterExt, default_aws_client};
///
/// #[tokio::main]
/// async fn main() -> kube_client::Result<()> {
///     let aws = default_aws_client().await;
///     let client = aws.try_eks_kube_client("my-cluster").await?;
///     Ok(())
/// }
/// ```
#[expect(async_fn_in_trait)]
pub trait TryEksClusterExt {
    /// Fetch the full [`aws_sdk_eks::types::Cluster`] metadata for the named cluster.
    ///
    /// This calls `eks:DescribeCluster` and returns the cluster object on success, or an
    /// [`aws_sdk_eks::Error::NotFoundException`] if the cluster does not exist.
    ///
    /// # Parameters
    ///
    /// * `cluster` — the EKS cluster name (e.g. `"my-cluster"`).
    ///
    /// # Errors
    ///
    /// Returns an [`aws_sdk_eks::Error`] if the AWS API call fails or the cluster is not found.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use kube_eks_config::{TryEksClusterExt, default_aws_client};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let aws = default_aws_client().await;
    ///     match aws.try_eks_cluster("my-cluster").await {
    ///         Ok(cluster) => println!("Cluster status: {:?}", cluster.status()),
    ///         Err(e) => eprintln!("Could not describe cluster: {e}"),
    ///     }
    /// }
    /// ```
    async fn try_eks_cluster(
        &self,
        cluster: impl Into<String>,
    ) -> Result<eks::types::Cluster, eks::Error>;

    /// Build a [`kube_client::Config`] for the named EKS cluster.
    ///
    /// Calls [`try_eks_cluster`](TryEksClusterExt::try_eks_cluster) and converts the result into
    /// a [`kube_client::Config`] via [`ToKubeConfig::into_kube_config`].
    ///
    /// Use this method when you need to inspect or further customise the config before creating
    /// a client (e.g. to set a default namespace).
    ///
    /// # Parameters
    ///
    /// * `cluster` — the EKS cluster name.
    ///
    /// # Errors
    ///
    /// Returns a [`kube_client::Error`] if the AWS API call fails or the cluster metadata cannot
    /// be converted into a valid Kubernetes configuration (e.g. missing endpoint).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use kube_eks_config::{TryEksClusterExt, default_aws_client};
    ///
    /// #[tokio::main]
    /// async fn main() -> kube_client::Result<()> {
    ///     let aws = default_aws_client().await;
    ///     let mut config = aws.try_eks_kube_config("my-cluster").await?;
    ///     config.default_namespace = "kube-system".to_owned();
    ///     let client = kube_client::Client::try_from(config)?;
    ///     Ok(())
    /// }
    /// ```
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

    /// Create a [`kube_client::Client`] connected to the named EKS cluster.
    ///
    /// This is the most convenient entry-point: it calls [`try_eks_kube_config`](TryEksClusterExt::try_eks_kube_config) and immediately
    /// wraps the resulting config in a [`kube_client::Client`].
    ///
    /// # Parameters
    ///
    /// * `cluster` — the EKS cluster name.
    ///
    /// # Errors
    ///
    /// Returns a [`kube_client::Error`] if the AWS API call fails, the cluster metadata is
    /// invalid, or the HTTP client cannot be initialised.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use kube_eks_config::{TryEksClusterExt, default_aws_client};
    ///
    /// #[tokio::main]
    /// async fn main() -> kube_client::Result<()> {
    ///     let aws = default_aws_client().await;
    ///     let client = aws.try_eks_kube_client("my-cluster").await?;
    ///     // Use `client` to make Kubernetes API calls
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

/// Conversion from an EKS cluster description into a [`kube_client::Config`].
///
/// This trait is implemented for [`aws_sdk_eks::types::Cluster`] and extracts the two pieces of
/// information needed to connect to a cluster:
///
/// * **Endpoint** — the HTTPS URL of the Kubernetes API server.
/// * **Certificate authority** — the base64-encoded PEM certificate used to verify TLS
///   connections to the API server.
///
/// ## Authentication note
///
/// EKS clusters use IAM-based authentication: the Kubernetes API server expects a bearer token
/// produced by running `aws eks get-token` (or the equivalent `aws-iam-authenticator`).  The
/// [`kube_client::Config`] produced here does **not** include such a token, so connecting with the
/// raw config will fail unless the cluster is configured to allow unauthenticated requests.
///
/// In practice, use the higher-level [`TryEksClusterExt::try_eks_kube_client`] together with an
/// IAM role that has the necessary `eks:DescribeCluster` permission.  The `kube` client picks up
/// any `exec`-based credential plugin already configured in `~/.kube/config`, but the config
/// returned by this crate is built from scratch and does **not** wire up a token-refresh exec
/// plugin automatically.  If your application needs long-running or refreshed IAM tokens, consider
/// layering a token provider (e.g. via [`kube_client::config::AuthInfo`]) on top of the returned
/// config before constructing the client.
pub trait ToKubeConfig {
    /// Convert this value into a [`kube_client::Config`].
    ///
    /// # Errors
    ///
    /// Returns a [`kube_client::config::KubeconfigError`] if the cluster's endpoint URL is
    /// missing or cannot be parsed.
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

/// Build an [`aws_sdk_eks::Error`] representing a "cluster not found" condition.
///
/// Used internally when `describe_cluster` returns a response with no cluster object.
fn cluster_not_found(cluster: &str) -> eks::Error {
    let exception = eks::types::error::NotFoundException::builder()
        .message(format!("EKS cluster {cluster} not found"))
        .build();
    eks::Error::NotFoundException(exception)
}

/// Create a default [`aws_sdk_eks::Client`] using the standard AWS environment configuration.
///
/// This is a convenience wrapper around [`aws_config::load_from_env`] followed by
/// [`aws_sdk_eks::Client::new`].  The AWS SDK will automatically discover credentials from
/// environment variables, the shared credentials file, IAM instance profiles, and other standard
/// sources in priority order.
///
/// For finer-grained control over the AWS configuration (e.g. to select a specific region or
/// credential profile) construct the [`aws_sdk_eks::Client`] manually:
///
/// ```no_run
/// async fn custom_client() -> aws_sdk_eks::Client {
///     let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
///         .region(aws_config::meta::region::RegionProviderChain::default_provider()
///             .or_else("eu-west-1"))
///         .load()
///         .await;
///     aws_sdk_eks::Client::new(&config)
/// }
/// ```
///
/// # Examples
///
/// ```no_run
/// use kube_eks_config::{TryEksClusterExt, default_aws_client};
///
/// #[tokio::main]
/// async fn main() -> kube_client::Result<()> {
///     let aws = default_aws_client().await;
///     let client = aws.try_eks_kube_client("my-cluster").await?;
///     Ok(())
/// }
/// ```
pub async fn default_aws_client() -> eks::Client {
    let config = aws_config::load_from_env().await;
    eks::Client::new(&config)
}
