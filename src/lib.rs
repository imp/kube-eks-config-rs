use kube_client::Error as KubeError;
use kube_client::Result as KubeResult;

pub use eks::ToKubeConfig;

mod eks;

#[expect(async_fn_in_trait)]
pub trait TryEks {
    type Output;

    async fn try_eks(cluster: impl Into<String>) -> KubeResult<Self::Output>;

    async fn try_eks_with_client(
        client: &eks::Client,
        cluster: impl Into<String>,
    ) -> KubeResult<Self::Output>;
}

impl TryEks for kube_client::Config {
    type Output = kube_client::Config;

    async fn try_eks(cluster: impl Into<String>) -> KubeResult<Self::Output> {
        let client = eks::default_client().await;
        Self::try_eks_with_client(&client, cluster).await
    }

    async fn try_eks_with_client(
        client: &eks::Client,
        cluster: impl Into<String>,
    ) -> KubeResult<Self::Output> {
        let cluster = cluster.into();
        eks::describe_cluster(client, &cluster)
            .await
            .map_err(|err| KubeError::Service(Box::new(err)))?
            .into_kube_config()
            .map_err(KubeError::InferKubeconfig)
    }
}

impl TryEks for kube_client::Client {
    type Output = kube_client::Client;

    async fn try_eks(cluster: impl Into<String>) -> KubeResult<Self::Output> {
        let client: aws_sdk_eks::Client = eks::default_client().await;
        Self::try_eks_with_client(&client, cluster).await
    }

    async fn try_eks_with_client(
        client: &eks::Client,
        cluster: impl Into<String>,
    ) -> KubeResult<Self::Output> {
        kube_client::Config::try_eks_with_client(client, cluster)
            .await
            .and_then(kube_client::Client::try_from)
    }
}
