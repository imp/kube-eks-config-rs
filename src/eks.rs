use aws_sdk_eks as eks;
use kube_client::config as kubeconfig;

pub use eks::Client;
pub use eks::Error;
pub use eks::types;

pub trait ToKubeConfig {
    fn into_kube_config(self) -> Result<kubeconfig::Config, kubeconfig::KubeconfigError>;
}

impl ToKubeConfig for types::Cluster {
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

pub(super) async fn describe_cluster(
    client: &Client,
    cluster: &str,
) -> Result<types::Cluster, Error> {
    client
        .describe_cluster()
        .name(cluster)
        .send()
        .await?
        .cluster
        .ok_or_else(|| cluster_not_found(cluster))
}

pub(crate) async fn default_client() -> Client {
    let config = aws_config::load_from_env().await;
    Client::new(&config)
}

fn cluster_not_found(cluster: &str) -> Error {
    let exception = types::error::NotFoundException::builder()
        .message(format!("EKS cluster {cluster} not found"))
        .build();
    Error::NotFoundException(exception)
}
