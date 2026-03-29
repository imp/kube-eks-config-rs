//! # listclusters
//!
//! Lists all EKS clusters visible to the current AWS identity in the configured region, then
//! prints each cluster's name, status, and Kubernetes version.
//!
//! ## Usage
//!
//! ```text
//! cargo run --example listclusters
//! ```
//!
//! AWS credentials must be available in the environment (environment variables, shared credentials
//! file, IAM role, etc.).  The IAM principal must have `eks:ListClusters` and
//! `eks:DescribeCluster` permissions.

use kube_eks_config::TryEksClusterExt;
use kube_eks_config::default_aws_client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build an EKS client from the ambient AWS environment (env vars, ~/.aws, IAM role, …)
    let aws = default_aws_client().await;

    // Retrieve the list of cluster names in the current region
    let names: Vec<String> = aws
        .list_clusters()
        .send()
        .await?
        .clusters
        .unwrap_or_default();

    if names.is_empty() {
        println!("No EKS clusters found in this region.");
        return Ok(());
    }

    println!("EKS clusters ({}):", names.len());
    for name in &names {
        // Fetch the full cluster description to access status and version
        match aws.try_eks_cluster(name).await {
            Ok(cluster) => {
                let status = cluster
                    .status()
                    .map(|s| s.as_str())
                    .unwrap_or("unknown");
                let version = cluster.version().unwrap_or("unknown");
                println!("  {name}  status={status}  k8s={version}");
            }
            Err(e) => eprintln!("  {name}  (error: {e})"),
        }
    }

    Ok(())
}
