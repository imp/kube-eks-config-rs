use aws_sdk_eks as eks;
use kube_client::Error as KubeError;
use kube_client::config as kubeconfig;

#[expect(async_fn_in_trait)]
pub trait TryEksClusterExt {
    async fn try_eks_cluster(
        &self,
        cluster: impl Into<String>,
    ) -> Result<eks::types::Cluster, eks::Error>;

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

pub trait ToKubeConfig {
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

fn cluster_not_found(cluster: &str) -> eks::Error {
    let exception = eks::types::error::NotFoundException::builder()
        .message(format!("EKS cluster {cluster} not found"))
        .build();
    eks::Error::NotFoundException(exception)
}

pub async fn default_aws_client() -> eks::Client {
    let config = aws_config::load_from_env().await;
    eks::Client::new(&config)
}
