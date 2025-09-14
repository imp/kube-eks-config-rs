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
    let aws = default_aws_client().await;
    if let Some(cluster) = args.next() {
        let config = aws.try_eks_kube_config(&cluster).await?;
        println!("{config:#?}");
        let client = aws.try_eks_kube_client(&cluster).await?;
        let lp = api::ListParams::default();
        api::Api::<corev1::Pod>::default_namespaced(client)
            .list(&lp)
            .await?
            .into_iter()
            .for_each(|pod| println!("{}", pod.name_any()));
    } else {
        println!("Usage: {cmd} <EKS cluster name>");
    }

    Ok(())
}
