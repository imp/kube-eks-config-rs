# kube-eks-config

[![Crates.io](https://img.shields.io/crates/v/kube-eks-config.svg)](https://crates.io/crates/kube-eks-config)
[![Docs.rs](https://docs.rs/kube-eks-config/badge.svg)](https://docs.rs/kube-eks-config)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Helpers for building a [`kube::Client`] directly from an [Amazon EKS](https://aws.amazon.com/eks/)
cluster name, without managing a kubeconfig file on disk.

## How it works

The crate calls the AWS EKS `DescribeCluster` API to retrieve the cluster's
HTTPS endpoint and certificate-authority data, then converts those values into
the configuration structs used by [`kube`]. Authentication (bearer
tokens) is intentionally omitted: EKS uses short-lived tokens obtained via
`aws eks get-token`, IRSA, or EKS Pod Identity — none of which belong in a
static config.

```
aws_sdk_eks::Client  ──►  EKS cluster name  ──►  kube::Config  ──►  kube::Client
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
kube-eks-config = "3.0"
kube = { version = "3.0", features = ["client", "rustls-tls"] }
tokio = { version = "1", features = ["full"] }
```

### One-shot: get a ready-to-use client

```rust,no_run
use kube_eks_config::{TryEksClusterExt, default_aws_client};

#[tokio::main]
async fn main() -> kube::Result<()> {
    let aws = default_aws_client().await;
    let client = aws.try_eks_kube_client("my-cluster").await?;
    // use `client` with kube::Api<T> …
    Ok(())
}
```

### Step by step

The three methods on `TryEksClusterExt` form a convenience ladder — use the
one that returns exactly what you need:

```rust,no_run
use kube_eks_config::{TryEksClusterExt, default_aws_client};

#[tokio::main]
async fn main() -> kube::Result<()> {
    let aws = default_aws_client().await;

    // 1. Raw cluster metadata (endpoint, version, tags, …)
    let cluster = aws
        .try_eks_cluster("my-cluster")
        .await
        .map_err(|e| kube::Error::Service(Box::new(e)))?;
    println!("Kubernetes version: {:?}", cluster.version);

    // 2. kube::Config (useful for customisation before building a client)
    let config = aws.try_eks_kube_config("my-cluster").await?;
    println!("Cluster URL: {}", config.cluster_url);

    // 3. Fully initialised client
    let client = kube::Client::try_from(config)?;
    let _ = client;
    Ok(())
}
```

### Producing a Kubeconfig struct

Use `IntoKubeconfig` when you need the serialisable kubeconfig format (e.g. to
write it to disk or merge it with an existing `~/.kube/config`):

```rust,no_run
use kube_eks_config::{IntoKubeconfig, TryEksClusterExt, default_aws_client};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let aws = default_aws_client().await;
    let cluster = aws
        .try_eks_cluster("my-cluster")
        .await
        .map_err(|e| kube::Error::Service(Box::new(e)))?;

    let kubeconfig = cluster.into_kubeconfig()?;
    let yaml = serde_yaml::to_string(&kubeconfig)?;
    std::fs::write("kubeconfig.yaml", yaml)?;
    Ok(())
}
```

## AWS credentials

`default_aws_client()` resolves credentials via the standard AWS provider chain
(highest priority first):

1. **Environment variables** — `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`,
   `AWS_SESSION_TOKEN`, `AWS_REGION` / `AWS_DEFAULT_REGION`
2. **AWS shared files** — `~/.aws/credentials` and `~/.aws/config`
3. **Web identity / IRSA** — `AWS_WEB_IDENTITY_TOKEN_FILE` + `AWS_ROLE_ARN`
   (Kubernetes pods with IAM Roles for Service Accounts)
4. **Instance metadata** — EC2 instance profile or ECS task role via IMDSv2

For fine-grained control over credentials, region, or endpoint configuration,
construct an `aws_sdk_eks::Client` directly and use `TryEksClusterExt` on it.

## Traits at a glance

| Trait | Input | Output |
|---|---|---|
| `TryEksClusterExt` | `eks::Client` + cluster name | raw cluster / `Config` / `Client` |
| `ToKubeConfig` | `eks::types::Cluster` | `kube::Config` |
| `IntoKubeconfig` | `eks::types::Cluster` | `kube::config::Kubeconfig` |

## Examples

| Example | Description |
|---|---|
| [`getpods-eks`](examples/getpods-eks.rs) | List pods using an EKS cluster name (uses this crate) |
| [`getpods`](examples/getpods.rs) | List pods using the kubeconfig already on disk (baseline) |

Run with:

```sh
cargo run --example getpods-eks -- <cluster-name>
```

## Versioning

The version of `kube-eks-config` tracks the **major version** of
[`kube`](https://crates.io/crates/kube). A `kube-eks-config 3.x.y` release is
compatible with any `kube 3.*` release, following standard semver rules.
When `kube` publishes a new major version, `kube-eks-config` will bump its
major version to match.

## License

Apache-2.0 — see [LICENSE](LICENSE) or <https://www.apache.org/licenses/LICENSE-2.0>.
