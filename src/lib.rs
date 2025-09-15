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

pub trait IntoKubeconfig {
    fn into_kubeconfig(self) -> Result<kubeconfig::Kubeconfig, kubeconfig::KubeconfigError>;
}

impl IntoKubeconfig for eks::types::Cluster {
    fn into_kubeconfig(self) -> Result<kubeconfig::Kubeconfig, kubeconfig::KubeconfigError> {
        let eks::types::Cluster {
            name,
            endpoint,
            certificate_authority,
            // arn,
            // created_at,
            // version,
            // role_arn,
            // resources_vpc_config,
            // kubernetes_network_config,
            // logging,
            // identity,
            // status,
            // client_request_token,
            // platform_version,
            // tags,
            // encryption_config,
            // connector_config,
            // id,
            // health,
            // outpost_config,
            // access_config,
            // upgrade_policy,
            // zonal_shift_config,
            // remote_network_config,
            // compute_config,
            // storage_config,
            // deletion_protection,
            ..
        } = self;
        let name = name.unwrap_or_else(|| "eks-cluster".to_string());
        let certificate_authority_data = certificate_authority.and_then(|cert| cert.data);

        let cluster = kubeconfig::Cluster {
            server: endpoint,
            insecure_skip_tls_verify: None,
            certificate_authority: None,
            certificate_authority_data,
            proxy_url: None,
            disable_compression: None,
            tls_server_name: None,
            extensions: None,
        };

        let named_cluster = kubeconfig::NamedCluster {
            name: name.clone(),
            cluster: Some(cluster),
        };

        let context = kubeconfig::Context {
            cluster: name.clone(),
            user: None,
            namespace: None,
            extensions: None,
        };

        let named_context = kubeconfig::NamedContext {
            name: name.clone(),
            context: Some(context),
        };

        let config = kubeconfig::Kubeconfig {
            clusters: vec![named_cluster],
            contexts: vec![named_context],
            current_context: Some(name),
            // auth_infos: vec![],
            ..kubeconfig::Kubeconfig::default()
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
