//! # getpods
//!
//! Lists all pods in the default namespace of an EKS cluster.
//!
//! ## Usage
//!
//! ```text
//! cargo run --example getpods -- <EKS cluster name>
//! ```
//!
//! AWS credentials must be available in the environment (environment variables, shared credentials
//! file, IAM role, etc.).  The IAM principal must have `eks:DescribeCluster` permission on the
//! named cluster, and the Kubernetes RBAC configuration must allow listing pods.

use std::env;

use k8s_openapi::api::core::v1 as corev1;
use kube::api;
use kube_client::ResourceExt;
use kube_eks_config::TryEksClusterExt;
use kube_eks_config::default_aws_client;

#[tokio::main]
async fn main() -> kube::Result<()> {
    let mut args = env::args();
    let cmd = args.next().unwrap();

    let Some(cluster) = args.next() else {
        eprintln!("Usage: {cmd} <EKS cluster name>");
        std::process::exit(1);
    };

    // Build an EKS client from the ambient AWS environment (env vars, ~/.aws, IAM role, …)
    let aws = default_aws_client().await;

    // try_eks_kube_client fetches the cluster metadata and builds both the Config and Client
    // in one step.  Use try_eks_kube_config instead if you need to inspect or customise the
    // Config first (e.g. to override the default namespace).
    let client = aws.try_eks_kube_client(&cluster).await?;

    // List pods in the default namespace
    let pods: Vec<_> = api::Api::<corev1::Pod>::default_namespaced(client)
        .list(&api::ListParams::default())
        .await?
        .into_iter()
        .collect();

    if pods.is_empty() {
        println!("No pods found in the default namespace.");
    } else {
        println!("Pods in the default namespace of '{cluster}':");
        for pod in pods {
            println!("  {}", pod.name_any());
        }
    }

    Ok(())
}
