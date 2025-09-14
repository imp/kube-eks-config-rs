use std::env;

use k8s_openapi::api::core::v1 as corev1;
use kube::api;
use kube_client::ResourceExt;
use kube_eks_config::TryEks;

#[tokio::main]
async fn main() -> kube::Result<()> {
    let mut args = env::args();
    let cmd = args.next().unwrap();
    if let Some(cluster) = args.next() {
        let config = kube::Config::try_eks(&cluster).await?;
        println!("{config:#?}");
        let client = kube::Client::try_eks(cluster).await?;
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
