use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

const RUNPOD_GRAPHQL_URL: &str = "https://api.runpod.io/graphql";
const RUNPOD_REST_BASE_URL: &str = "https://rest.runpod.io/v1";

/// Main client struct for interacting with RunPod.
pub struct RunpodClient {
    http_client: Client,
    api_key: String,
}

impl RunpodClient {
    /// Construct a new RunpodClient with your API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        RunpodClient {
            http_client: Client::new(),
            api_key: api_key.into(),
        }
    }

    // -------------------------------------------------------------------------
    //  A) GRAPHQL CALLS (for features not yet in REST)
    // -------------------------------------------------------------------------
    async fn graphql_query<T: for<'de> Deserialize<'de>>(
        &self,
        graphql_body: &Value,
    ) -> Result<T, reqwest::Error> {
        // GraphQL: can use `?api_key=` or an HTTP header; this example uses ?api_key=
        let url = format!("{}?api_key={}", RUNPOD_GRAPHQL_URL, self.api_key);
        let resp = self
            .http_client
            .post(&url)
            .json(graphql_body)
            .send()
            .await?
            .error_for_status()?
            .json::<T>()
            .await?;
        Ok(resp)
    }

    /// List GPU Types (only available in GraphQL).
    pub async fn list_gpu_types_graphql(&self) -> Result<GPUTypesListResponseData, reqwest::Error> {
        let query = r#"
            query GpuTypes {
                gpuTypes {
                    id
                    displayName
                    memoryInGb
                }
            }
        "#;
        let body = json!({ "query": query });

        let resp: GraphQLResponse<GPUTypesListResponse> = self.graphql_query(&body).await?;
        Ok(GPUTypesListResponseData {
            data: resp.data.map(|d| d.gpu_types),
            errors: resp.errors,
        })
    }

    /// Get GPU Type by ID (only available in GraphQL).
    pub async fn get_gpu_type_graphql(
        &self,
        gpu_type_id: &str,
    ) -> Result<GPUTypeResponseData, reqwest::Error> {
        let query = format!(
            r#"
            query GpuTypes {{
                gpuTypes(input: {{
                    id: "{gpu_type_id}"
                }}) {{
                    id
                    displayName
                    memoryInGb
                    secureCloud
                    communityCloud
                    lowestPrice(input: {{gpuCount: 1}}) {{
                        minimumBidPrice
                        uninterruptablePrice
                    }}
                }}
            }}
            "#,
            gpu_type_id = gpu_type_id
        );
        let body = json!({ "query": query });

        let resp: GraphQLResponse<GPUTypesExtendedResponse> = self.graphql_query(&body).await?;
        Ok(GPUTypeResponseData {
            data: resp.data.map(|d| d.gpu_types),
            errors: resp.errors,
        })
    }

    // -------------------------------------------------------------------------
    //  B) REST CALLS (for Pods, Network Volumes, etc.)
    // -------------------------------------------------------------------------
    fn rest_request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}/{}", RUNPOD_REST_BASE_URL, path);
        self.http_client
            .request(method, &url)
            .bearer_auth(&self.api_key)
    }

    // -------------------------------------------------------------------------
    //  B.1) Pods
    // -------------------------------------------------------------------------
    /// Create an **on-demand** (reserved) Pod using the REST API.
    pub async fn create_on_demand_pod_rest(
        &self,
        req: CreateOnDemandPodRequest,
    ) -> Result<PodCreateResponseData, reqwest::Error> {
        // Convert your custom request -> PodCreateInput
        let payload = req.to_pod_create_input(false);
        let resp = self
            .rest_request(Method::POST, "pods")
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;

        let pod = resp.json::<Pod>().await?;
        Ok(PodCreateResponseData {
            data: Some(pod),
            errors: None,
        })
    }

    /// Create a **spot/interruptible** Pod using the REST API.
    pub async fn create_spot_pod_rest(
        &self,
        req: CreateSpotPodRequest,
    ) -> Result<PodCreateResponseData, reqwest::Error> {
        let payload = req.to_pod_create_input(true);
        let resp = self
            .rest_request(Method::POST, "pods")
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;

        let pod = resp.json::<Pod>().await?;
        Ok(PodCreateResponseData {
            data: Some(pod),
            errors: None,
        })
    }

    /// Start (Resume) a Pod. REST does not differentiate on-demand vs. spot.
    pub async fn start_pod_rest(
        &self,
        pod_id: &str,
    ) -> Result<PodStartResponseData, reqwest::Error> {
        let path = format!("pods/{pod_id}/start");
        let resp = self
            .rest_request(Method::POST, &path)
            .send()
            .await?
            .error_for_status()?;

        let pod = resp.json::<Pod>().await?;
        Ok(PodStartResponseData {
            data: Some(pod.into_minimal()),
            errors: None,
        })
    }

    /// Stop a Pod.
    pub async fn stop_pod_rest(&self, pod_id: &str) -> Result<PodStopResponseData, reqwest::Error> {
        let path = format!("pods/{pod_id}/stop");
        let resp = self
            .rest_request(Method::POST, &path)
            .send()
            .await?
            .error_for_status()?;

        let pod = resp.json::<Pod>().await?;
        Ok(PodStopResponseData {
            data: Some(PodInfoMinimalStop {
                id: pod.id,
                desired_status: pod.desired_status,
            }),
            errors: None,
        })
    }

    /// List all Pods.
    pub async fn list_pods_rest(&self) -> Result<PodsListResponseData, reqwest::Error> {
        let resp = self
            .rest_request(Method::GET, "pods")
            .send()
            .await?
            .error_for_status()?;

        let pods = resp.json::<Vec<Pod>>().await?;
        let my_pods = MyselfPods {
            pods: pods
                .into_iter()
                .map(|p| PodInfoFull {
                    id: p.id,
                    name: p.name.unwrap_or_default(),
                    runtime: None,
                })
                .collect(),
        };
        Ok(PodsListResponseData {
            data: Some(my_pods),
            errors: None,
        })
    }

    /// Get a Pod by ID.
    pub async fn get_pod_rest(&self, pod_id: &str) -> Result<PodInfoResponseData, reqwest::Error> {
        let path = format!("pods/{pod_id}");
        let resp = self
            .rest_request(Method::GET, &path)
            .send()
            .await?
            .error_for_status()?;

        let pod = resp.json::<Pod>().await?;
        Ok(PodInfoResponseData {
            data: Some(PodInfoFull {
                id: pod.id,
                name: pod.name.unwrap_or_default(),
                runtime: None,
            }),
            errors: None,
        })
    }

    // -------------------------------------------------------------------------
    //  B.2) Network Volumes
    //    - The OpenAPI shows these endpoints:
    //      POST   /networkvolumes
    //      GET    /networkvolumes
    //      GET    /networkvolumes/{networkVolumeId}
    //      PATCH  /networkvolumes/{networkVolumeId}
    //      DELETE /networkvolumes/{networkVolumeId}
    // -------------------------------------------------------------------------

    /// Create a new **Network Volume**.
    pub async fn create_network_volume(
        &self,
        req: NetworkVolumeCreateInput,
    ) -> Result<NetworkVolume, reqwest::Error> {
        let resp = self
            .rest_request(Method::POST, "networkvolumes")
            .json(&req)
            .send()
            .await?
            .error_for_status()?;

        let vol = resp.json::<NetworkVolume>().await?;
        Ok(vol)
    }

    /// List all **Network Volumes**.
    pub async fn list_network_volumes(&self) -> Result<Vec<NetworkVolume>, reqwest::Error> {
        let resp = self
            .rest_request(Method::GET, "networkvolumes")
            .send()
            .await?
            .error_for_status()?;

        // According to the spec, it returns an array of `NetworkVolume`.
        let volumes = resp.json::<Vec<NetworkVolume>>().await?;
        Ok(volumes)
    }

    /// Get a specific Network Volume by ID.
    pub async fn get_network_volume(
        &self,
        network_volume_id: &str,
    ) -> Result<NetworkVolume, reqwest::Error> {
        let path = format!("networkvolumes/{}", network_volume_id);
        let resp = self
            .rest_request(Method::GET, &path)
            .send()
            .await?
            .error_for_status()?;

        let vol = resp.json::<NetworkVolume>().await?;
        Ok(vol)
    }

    /// Update (rename or resize) a Network Volume.  
    /// For resizing, you must specify a `size` **greater** than the current size.
    pub async fn update_network_volume(
        &self,
        network_volume_id: &str,
        req: NetworkVolumeUpdateInput,
    ) -> Result<NetworkVolume, reqwest::Error> {
        let path = format!("networkvolumes/{}", network_volume_id);
        let resp = self
            .rest_request(Method::PATCH, &path)
            .json(&req)
            .send()
            .await?
            .error_for_status()?;

        let vol = resp.json::<NetworkVolume>().await?;
        Ok(vol)
    }

    /// Delete a Network Volume by ID.
    pub async fn delete_network_volume(
        &self,
        network_volume_id: &str,
    ) -> Result<(), reqwest::Error> {
        let path = format!("networkvolumes/{}", network_volume_id);
        self.rest_request(Method::DELETE, &path)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// GraphQL Data Structures
// -----------------------------------------------------------------------------
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GraphQLResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GraphQLError {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GPUTypesListResponse {
    #[serde(rename = "gpuTypes")]
    pub gpu_types: Vec<GpuTypeMinimal>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GPUTypesListResponseData {
    pub data: Option<Vec<GpuTypeMinimal>>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GPUTypeResponseData {
    pub data: Option<Vec<GpuTypeExtended>>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GPUTypesExtendedResponse {
    #[serde(rename = "gpuTypes")]
    pub gpu_types: Vec<GpuTypeExtended>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GpuTypeMinimal {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "memoryInGb")]
    pub memory_in_gb: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GpuTypeExtended {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "memoryInGb")]
    pub memory_in_gb: Option<i32>,
    #[serde(rename = "secureCloud")]
    pub secure_cloud: Option<bool>,
    #[serde(rename = "communityCloud")]
    pub community_cloud: Option<bool>,
    pub lowest_price: Option<LowestPrice>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LowestPrice {
    #[serde(rename = "minimumBidPrice")]
    pub minimum_bid_price: Option<f64>,
    #[serde(rename = "uninterruptablePrice")]
    pub uninterruptable_price: Option<f64>,
}

// -----------------------------------------------------------------------------
// REST Data Structures
// -----------------------------------------------------------------------------

//
// 1) Pods
//
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PodCreateInput {
    pub name: Option<String>,
    pub locked: Option<bool>,
    pub image_name: Option<String>,
    pub container_registry_auth_id: Option<String>,

    pub container_disk_in_gb: Option<i32>,
    pub docker_start_cmd: Option<Vec<String>>,
    pub docker_entrypoint: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub ports: Option<Vec<String>>,

    pub volume_in_gb: Option<i32>,
    pub volume_mount_path: Option<String>,

    pub allowed_cuda_versions: Option<Vec<String>>,
    pub bid_per_gpu: Option<f64>,
    pub cloud_type: Option<String>,
    pub country_codes: Option<Vec<String>>,
    pub data_center_ids: Option<Vec<String>>,
    pub gpu_type_ids: Option<Vec<String>>,
    pub min_ram_per_gpu: Option<i32>,
    pub min_vcpu_per_gpu: Option<i32>,
    pub gpu_type_priority: Option<String>,
    pub data_center_priority: Option<String>,
    pub support_public_ip: Option<bool>,
    pub min_download_mbps: Option<i32>,
    pub min_upload_mbps: Option<i32>,
    pub min_disk_bandwidth_mbps: Option<i32>,
    pub template_id: Option<String>,
    pub network_volume_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pod {
    pub id: String,
    pub name: Option<String>,
    pub image: Option<String>,
    pub desired_status: Option<String>,
    // Add other fields if you like
}

impl Pod {
    pub fn into_minimal(self) -> PodInfoMinimal {
        PodInfoMinimal {
            id: self.id,
            image_name: self.image.unwrap_or_default(),
            env: vec![],
            machine_id: String::new(),
            machine: MachineHost { pod_host_id: None },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodCreateResponseData {
    pub data: Option<Pod>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodStartResponseData {
    pub data: Option<PodInfoMinimal>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodStopResponseData {
    pub data: Option<PodInfoMinimalStop>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodsListResponseData {
    pub data: Option<MyselfPods>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodInfoResponseData {
    pub data: Option<PodInfoFull>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct MyselfPods {
    pub pods: Vec<PodInfoFull>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodInfoMinimal {
    pub id: String,
    #[serde(rename = "imageName")]
    pub image_name: String,
    pub env: Vec<EnvVar>,
    #[serde(rename = "machineId")]
    pub machine_id: String,
    pub machine: MachineHost,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodInfoMinimalStop {
    pub id: String,
    #[serde(rename = "desiredStatus")]
    pub desired_status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodInfoFull {
    pub id: String,
    pub name: String,
    pub runtime: Option<PodRuntime>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodRuntime {
    #[serde(rename = "uptimeInSeconds")]
    pub uptime_in_seconds: Option<i64>,
    // etc
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct MachineHost {
    #[serde(rename = "podHostId")]
    pub pod_host_id: Option<String>,
}

// -----------------------------------------------------------------------------
// 2) Network Volumes
// -----------------------------------------------------------------------------
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkVolume {
    pub id: String,
    pub name: String,
    pub size: i32,
    pub data_center_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkVolumeCreateInput {
    pub name: String,
    pub size: i32,
    pub data_center_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkVolumeUpdateInput {
    // If provided, rename the volume.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    // If provided, resize the volume (size must be > current size).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<i32>,
}

// -----------------------------------------------------------------------------
// 3) Conversion logic for your "CreateOnDemandPodRequest" and "CreateSpotPodRequest"
//    into the REST "PodCreateInput"
// -----------------------------------------------------------------------------
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CreateOnDemandPodRequest {
    pub cloud_type: String,
    pub gpu_count: i32, // not directly used by REST
    pub volume_in_gb: i32,
    pub container_disk_in_gb: i32,
    pub min_vcpu_count: i32,
    pub min_memory_in_gb: i32,
    pub gpu_type_id: String,
    pub name: String,
    pub image_name: String,
    pub docker_args: String,
    pub ports: String,
    pub volume_mount_path: String,
    pub env: Vec<EnvVar>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CreateSpotPodRequest {
    pub bid_per_gpu: f64,
    pub cloud_type: String,
    pub gpu_count: i32,
    pub volume_in_gb: i32,
    pub container_disk_in_gb: i32,
    pub min_vcpu_count: i32,
    pub min_memory_in_gb: i32,
    pub gpu_type_id: String,
    pub name: String,
    pub image_name: String,
    pub docker_args: String,
    pub ports: String,
    pub volume_mount_path: String,
    pub env: Vec<EnvVar>,
}

impl CreateOnDemandPodRequest {
    pub fn to_pod_create_input(&self, is_spot: bool) -> PodCreateInput {
        let cloud_type = match self.cloud_type.as_str() {
            "SECURE" => "SECURE".to_string(),
            "COMMUNITY" => "COMMUNITY".to_string(),
            _ => "SECURE".to_string(),
        };
        let gpu_type_ids = if self.gpu_type_id.is_empty() {
            vec![]
        } else {
            vec![self.gpu_type_id.clone()]
        };
        let ports: Vec<String> = self
            .ports
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let env_map: HashMap<String, String> = self
            .env
            .iter()
            .map(|v| (v.key.clone(), v.value.clone()))
            .collect();

        PodCreateInput {
            name: Some(self.name.clone()),
            image_name: Some(self.image_name.clone()),
            container_disk_in_gb: Some(self.container_disk_in_gb),
            volume_in_gb: Some(self.volume_in_gb),
            volume_mount_path: Some(self.volume_mount_path.clone()),
            ports: Some(ports),
            env: Some(env_map),
            docker_start_cmd: if self.docker_args.is_empty() {
                None
            } else {
                Some(
                    self.docker_args
                        .split_whitespace()
                        .map(|s| s.to_string())
                        .collect(),
                )
            },
            docker_entrypoint: None,
            locked: None,
            container_registry_auth_id: None,
            allowed_cuda_versions: None,
            cloud_type: Some(cloud_type),
            data_center_ids: None,
            gpu_type_ids: Some(gpu_type_ids),
            min_ram_per_gpu: Some(self.min_memory_in_gb),
            min_vcpu_per_gpu: Some(self.min_vcpu_count),
            network_volume_id: None,
            bid_per_gpu: if is_spot { Some(0.0) } else { None },
            country_codes: None,
            gpu_type_priority: None,
            data_center_priority: None,
            support_public_ip: None,
            min_download_mbps: None,
            min_upload_mbps: None,
            min_disk_bandwidth_mbps: None,
            template_id: None,
        }
    }
}

impl CreateSpotPodRequest {
    pub fn to_pod_create_input(&self, _is_spot: bool) -> PodCreateInput {
        let cloud_type = match self.cloud_type.as_str() {
            "SECURE" => "SECURE".to_string(),
            "COMMUNITY" => "COMMUNITY".to_string(),
            _ => "SECURE".to_string(),
        };
        let gpu_type_ids = if self.gpu_type_id.is_empty() {
            vec![]
        } else {
            vec![self.gpu_type_id.clone()]
        };
        let ports: Vec<String> = self
            .ports
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let env_map: HashMap<String, String> = self
            .env
            .iter()
            .map(|v| (v.key.clone(), v.value.clone()))
            .collect();

        PodCreateInput {
            name: Some(self.name.clone()),
            image_name: Some(self.image_name.clone()),
            container_disk_in_gb: Some(self.container_disk_in_gb),
            volume_in_gb: Some(self.volume_in_gb),
            volume_mount_path: Some(self.volume_mount_path.clone()),
            ports: Some(ports),
            env: Some(env_map),
            docker_start_cmd: if self.docker_args.is_empty() {
                None
            } else {
                Some(
                    self.docker_args
                        .split_whitespace()
                        .map(|s| s.to_string())
                        .collect(),
                )
            },
            docker_entrypoint: None,
            locked: None,
            container_registry_auth_id: None,
            allowed_cuda_versions: None,
            cloud_type: Some(cloud_type),
            data_center_ids: None,
            gpu_type_ids: Some(gpu_type_ids),
            min_ram_per_gpu: Some(self.min_memory_in_gb),
            min_vcpu_per_gpu: Some(self.min_vcpu_count),
            network_volume_id: None,
            // Spot => set bidPerGpu
            bid_per_gpu: Some(self.bid_per_gpu),
            country_codes: None,
            gpu_type_priority: None,
            data_center_priority: None,
            support_public_ip: None,
            min_download_mbps: None,
            min_upload_mbps: None,
            min_disk_bandwidth_mbps: None,
            template_id: None,
        }
    }
}
