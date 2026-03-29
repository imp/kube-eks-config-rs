//! Baseline example: list pods using the kubeconfig already present on disk
//! (`~/.kube/config` or `KUBECONFIG` env var).  This does **not** use
//! `kube-eks-config`; it is here as a comparison point to show what the crate
//! saves you from doing manually.

use k8s_openapi::api::core::v1 as corev1;
use kube::api;
use kube_client::ResourceExt;

#[tokio::main]
async fn main() -> kube::Result<()> {
    let client = kube::Client::try_default().await?;
    let lp = api::ListParams::default();
    api::Api::<corev1::Pod>::default_namespaced(client)
        .list(&lp)
        .await?
        .into_iter()
        .for_each(|pod| println!("{}", pod.name_any()));

    Ok(())
}
