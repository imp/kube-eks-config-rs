//! Example: list pods in the default namespace of an Amazon EKS cluster.
//!
//! AWS credentials are resolved automatically from the environment
//! (env vars, `~/.aws/credentials`, IRSA, EC2 metadata, etc.).
//!
//! Usage:
//! ```text
//! cargo run --example getpods-eks -- <EKS-cluster-name>
//! ```

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
        return Ok(());
    };

    let aws = default_aws_client().await;

    // Step 1 (optional): inspect the raw cluster metadata from AWS.
    let cluster_info = aws
        .try_eks_cluster(&cluster)
        .await
        .map_err(|err| kube::Error::Service(Box::new(err)))?;
    println!("Cluster endpoint : {:?}", cluster_info.endpoint);
    println!("Kubernetes version: {:?}", cluster_info.version);

    // Step 2 (optional): inspect the derived kube config.
    let config = aws.try_eks_kube_config(&cluster).await?;
    println!("kube::Config URL  : {}", config.cluster_url);

    // Step 3: build the client and list pods.
    let client = aws.try_eks_kube_client(&cluster).await?;
    let lp = api::ListParams::default();
    let pods = api::Api::<corev1::Pod>::default_namespaced(client)
        .list(&lp)
        .await?;
    println!("\nPods in default namespace:");
    for pod in pods {
        println!("  {}", pod.name_any());
    }

    Ok(())
}
