use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::info;
const RUNPOD_GRAPHQL_URL: &str = "https://api.runpod.io/graphql";
const RUNPOD_REST_BASE_URL: &str = "https://rest.runpod.io/v1";

/// Main client struct for interacting with RunPod.
#[derive(Clone)]
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
        query: &str,
    ) -> Result<T, reqwest::Error> {
        // Log the request we're about to make (without the API key)
        info!("[RunPod] Making GraphQL request to {}", RUNPOD_GRAPHQL_URL);

        // Format the request body like the Python implementation
        let body = json!({ "query": query });

        // Create the request with proper headers
        let request = self
            .http_client
            .post(RUNPOD_GRAPHQL_URL)
            .header("Content-Type", "application/json")
            .header("User-Agent", "Rust-RunPod-Client/1.0")
            .bearer_auth(&self.api_key)
            .json(&body);

        // Send the request and get the response
        let response = request.send().await?;

        // Log the status code
        info!("[RunPod] GraphQL response status: {}", response.status());

        // Check for errors
        let response = response.error_for_status()?;

        // Parse the JSON response
        let parsed = response.json::<T>().await?;
        Ok(parsed)
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

        let resp: GraphQLResponse<GPUTypesListResponse> = self.graphql_query(query).await?;
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

        let resp: GraphQLResponse<GPUTypesExtendedResponse> = self.graphql_query(&query).await?;
        Ok(GPUTypeResponseData {
            data: resp.data.map(|d| d.gpu_types),
            errors: resp.errors,
        })
    }

    pub async fn list_available_gpus_with_datacenters(
        &self,
    ) -> Result<DatacentersWithGpuTypes, reqwest::Error> {
        let query = r#"
            query myself {
                myself {
                    datacenters {
                        id
                        name
                        location
                        available
                        gpuTypes {
                            id
                            displayName
                            memoryInGb
                            available
                            secureCloud
                            communityCloud
                            securePrice
                            communityPrice
                            communitySpotPrice
                            secureSpotPrice
                        }
                    }
                }
            }
        "#;

        let resp: GraphQLResponse<MyselfResponse> = self.graphql_query(query).await?;

        Ok(DatacentersWithGpuTypes {
            data: resp.data.map(|d| d.myself.datacenters),
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
    pub async fn create_on_demand_pod(
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
    pub async fn create_spot_pod(
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
    pub async fn start_pod(&self, pod_id: &str) -> Result<PodStartResponseData, reqwest::Error> {
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
    pub async fn stop_pod(&self, pod_id: &str) -> Result<PodStopResponseData, reqwest::Error> {
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
                desired_status: Some(pod.desired_status),
            }),
            errors: None,
        })
    }

    /// List all Pods.
    pub async fn list_pods(&self) -> Result<PodsListResponseData, reqwest::Error> {
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
                    desired_status: p.desired_status,
                    last_status_change: p.last_status_change,
                    machine_id: p.machine_id,
                    gpu_count: p.gpu.as_ref().map(|g| g.count).unwrap_or_default(),
                    memory_in_gb: p.memory_in_gb,
                    vcpu_count: p.vcpu_count,
                    storage: PodStorage {
                        container_disk_in_gb: p.container_disk_in_gb,
                        volume_in_gb: p.volume_in_gb,
                        volume_mount_path: p.volume_mount_path.unwrap_or_default(),
                        volume_encrypted: p.volume_encrypted,
                    },
                    cost_per_hr: p.cost_per_hr.parse().unwrap_or_default(),
                    adjusted_cost_per_hr: p.adjusted_cost_per_hr,
                    interruptible: p.interruptible,
                    ports: p.ports.clone(),
                    public_ip: p.public_ip.clone(),
                    image: p.image.clone(),
                    env: p.env.clone(),
                    last_started_at: p.last_started_at.clone(),
                    network_volume: p.network_volume.map(|nv| NetworkVolume {
                        id: nv.id,
                        name: nv.name,
                        size: nv.size,
                        data_center_id: nv.data_center_id,
                    }),
                    machine: p.machine.map(|m| PodMachineInfo {
                        id: m.id,
                        location: m.location,
                        gpu_display_name: m.gpu_display_name,
                        support_public_ip: m.support_public_ip,
                    }),
                })
                .collect(),
        };
        Ok(PodsListResponseData {
            data: Some(my_pods),
            errors: None,
        })
    }

    /// Get a Pod by ID.
    pub async fn get_pod(&self, pod_id: &str) -> Result<PodInfoResponseData, reqwest::Error> {
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
                desired_status: pod.desired_status,
                last_status_change: pod.last_status_change,
                machine_id: pod.machine_id,
                gpu_count: pod.gpu.as_ref().map(|g| g.count).unwrap_or_default(),
                memory_in_gb: pod.memory_in_gb,
                vcpu_count: pod.vcpu_count,
                storage: PodStorage {
                    container_disk_in_gb: pod.container_disk_in_gb,
                    volume_in_gb: pod.volume_in_gb,
                    volume_mount_path: pod.volume_mount_path.unwrap_or_default(),
                    volume_encrypted: pod.volume_encrypted,
                },
                runtime: pod.runtime.map(|r| PodRuntime {
                    uptime_in_seconds: r.uptime_in_seconds.unwrap_or_default(),
                    // Add other runtime fields as needed
                }),
                cost_per_hr: pod.cost_per_hr.parse().unwrap_or_default(),
                adjusted_cost_per_hr: pod.adjusted_cost_per_hr,
                interruptible: pod.interruptible,
                ports: pod.ports.clone(),
                public_ip: pod.public_ip.clone(),
                image: pod.image.clone(),
                env: pod.env.clone(),
                last_started_at: pod.last_started_at.clone(),
                network_volume: pod.network_volume.map(|nv| NetworkVolume {
                    id: nv.id,
                    name: nv.name,
                    size: nv.size,
                    data_center_id: nv.data_center_id,
                }),
                machine: pod.machine.map(|m| PodMachineInfo {
                    id: m.id,
                    location: m.location,
                    gpu_display_name: m.gpu_display_name,
                    support_public_ip: m.support_public_ip,
                    // Add other machine fields as needed
                }),
                // Add other fields as needed
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

    // -------------------------------------------------------------------------
    //  B.3) Endpoints
    // -------------------------------------------------------------------------
    /// List all Endpoints.
    pub async fn list_endpoints(&self) -> Result<Endpoints, reqwest::Error> {
        let resp = self
            .rest_request(Method::GET, "endpoints")
            .send()
            .await?
            .error_for_status()?;

        // According to the spec, it returns an array of `Endpoint`.
        // e.g. JSON -> [ {...}, {...} ]
        let endpoints = resp.json::<Endpoints>().await?;
        Ok(endpoints)
    }

    /// Get a specific Endpoint by ID.
    pub async fn get_endpoint(&self, endpoint_id: &str) -> Result<Endpoint, reqwest::Error> {
        let path = format!("endpoints/{}", endpoint_id);
        let resp = self
            .rest_request(Method::GET, &path)
            .send()
            .await?
            .error_for_status()?;

        let endpoint = resp.json::<Endpoint>().await?;
        Ok(endpoint)
    }

    /// Create a new Endpoint.
    pub async fn create_endpoint(
        &self,
        req: EndpointCreateInput,
    ) -> Result<Endpoint, reqwest::Error> {
        let resp = self
            .rest_request(Method::POST, "endpoints")
            .json(&req)
            .send()
            .await?
            .error_for_status()?;

        let endpoint = resp.json::<Endpoint>().await?;
        Ok(endpoint)
    }

    /// Update (patch) an existing Endpoint by ID.
    pub async fn update_endpoint(
        &self,
        endpoint_id: &str,
        req: EndpointUpdateInput,
    ) -> Result<Endpoint, reqwest::Error> {
        let path = format!("endpoints/{}", endpoint_id);
        let resp = self
            .rest_request(Method::PATCH, &path)
            .json(&req)
            .send()
            .await?
            .error_for_status()?;

        let endpoint = resp.json::<Endpoint>().await?;
        Ok(endpoint)
    }

    /// Delete an Endpoint by ID.
    pub async fn delete_endpoint(&self, endpoint_id: &str) -> Result<(), reqwest::Error> {
        let path = format!("endpoints/{}", endpoint_id);
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

#[derive(Debug, Deserialize)]
pub struct MyselfResponse {
    pub myself: MyselfData,
}

#[derive(Debug, Deserialize)]
pub struct MyselfData {
    pub datacenters: Vec<DatacenterWithGpuTypes>,
}

#[derive(Debug, Deserialize)]
pub struct DatacenterWithGpuTypes {
    pub id: String,
    pub name: String,
    pub location: String,
    pub available: bool,
    #[serde(rename = "gpuTypes")]
    pub gpu_types: Vec<DatacenterGpuType>,
}

#[derive(Debug, Deserialize)]
pub struct DatacenterGpuType {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "memoryInGb")]
    pub memory_in_gb: f64,
    pub available: i32,
    #[serde(rename = "secureCloud")]
    pub secure_cloud: bool,
    #[serde(rename = "communityCloud")]
    pub community_cloud: bool,
    #[serde(rename = "securePrice")]
    pub secure_price: Option<f64>,
    #[serde(rename = "communityPrice")]
    pub community_price: Option<f64>,
    #[serde(rename = "communitySpotPrice")]
    pub community_spot_price: Option<f64>,
    #[serde(rename = "secureSpotPrice")]
    pub secure_spot_price: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct DatacentersWithGpuTypes {
    pub data: Option<Vec<DatacenterWithGpuTypes>>,
    pub errors: Option<Vec<GraphQLError>>,
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

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Pod {
    pub id: String,
    pub name: Option<String>,
    pub desired_status: String,
    pub last_status_change: String,
    pub machine_id: String,
    pub memory_in_gb: f64,
    pub vcpu_count: f64,
    pub container_disk_in_gb: i32,
    pub volume_in_gb: f64,
    pub volume_mount_path: Option<String>,
    pub volume_encrypted: bool,
    pub cost_per_hr: String,
    pub adjusted_cost_per_hr: f64,
    pub interruptible: bool,
    pub ports: Vec<String>,
    pub public_ip: Option<String>,
    pub image: String,
    pub env: HashMap<String, String>,
    pub last_started_at: Option<String>,
    pub gpu: Option<GpuInfo>,
    pub machine: Option<MachineInfo>,
    pub runtime: Option<RuntimeInfo>,
    pub network_volume: Option<NetworkVolumeInfo>,
    pub savings_plans: Option<Vec<SavingsPlan>>,
    // Add other fields as needed
}

impl Pod {
    pub fn into_minimal(self) -> PodInfoMinimal {
        PodInfoMinimal {
            id: self.id,
            image_name: self.image,
            env: vec![],
            machine_id: String::new(),
            machine: MachineHost { pod_host_id: None },
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub id: String,
    pub count: i32,
    pub display_name: String,
    // Add other GPU fields as needed
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MachineInfo {
    pub id: String,
    pub location: String,
    pub gpu_display_name: String,
    pub support_public_ip: bool,
    pub secure_cloud: bool,
    // Add other machine fields as needed
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub uptime_in_seconds: Option<i32>,
    // Add other runtime fields as needed
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NetworkVolumeInfo {
    pub id: String,
    pub name: String,
    pub size: i32,
    pub data_center_id: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SavingsPlan {
    pub id: String,
    pub pod_id: String,
    pub gpu_type_id: String,
    pub cost_per_hr: f64,
    pub start_time: String,
    pub end_time: String,
}

// Update the PodInfoFull struct to include all the fields
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PodInfoFull {
    pub id: String,
    pub name: String,
    pub desired_status: String,
    pub last_status_change: String,
    pub machine_id: String,
    pub gpu_count: i32,
    pub memory_in_gb: f64,
    pub vcpu_count: f64,
    pub storage: PodStorage,
    pub runtime: Option<PodRuntime>,
    pub cost_per_hr: f64,
    pub adjusted_cost_per_hr: f64,
    pub interruptible: bool,
    pub ports: Vec<String>,
    pub public_ip: Option<String>,
    pub image: String,
    pub env: HashMap<String, String>,
    pub last_started_at: Option<String>,
    pub network_volume: Option<NetworkVolume>,
    pub machine: Option<PodMachineInfo>,
    // Add other fields as needed
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PodStorage {
    pub container_disk_in_gb: i32,
    pub volume_in_gb: f64,
    pub volume_mount_path: String,
    pub volume_encrypted: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PodRuntime {
    pub uptime_in_seconds: i32,
    // Add other runtime fields as needed
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NetworkVolume {
    pub id: String,
    pub name: String,
    pub size: i32,
    pub data_center_id: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PodMachineInfo {
    pub id: String,
    pub location: String,
    pub gpu_display_name: String,
    pub support_public_ip: bool,
    // Add other machine fields as needed
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PodCreateResponseData {
    pub data: Option<Pod>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PodStartResponseData {
    pub data: Option<PodInfoMinimal>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PodStopResponseData {
    pub data: Option<PodInfoMinimalStop>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PodsListResponseData {
    pub data: Option<MyselfPods>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PodInfoResponseData {
    pub data: Option<PodInfoFull>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MyselfPods {
    pub pods: Vec<PodInfoFull>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct PodInfoMinimalStop {
    pub id: String,
    #[serde(rename = "desiredStatus")]
    pub desired_status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MachineHost {
    #[serde(rename = "podHostId")]
    pub pod_host_id: Option<String>,
}

// -----------------------------------------------------------------------------
// 2) Network Volumes
// -----------------------------------------------------------------------------

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

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct EndpointResponseData {
    pub data: Option<Endpoint>,
    pub errors: Option<Vec<GraphQLError>>, // or your error structure
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct EndpointsListResponseData {
    pub data: Option<Vec<Endpoint>>,
    pub errors: Option<Vec<GraphQLError>>,
}

//
// 3) Endpoints
//
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    /// Unique ID of the endpoint
    pub id: String,

    /// The user who created the endpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,

    /// A user-defined name for a Serverless endpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The latest version of a Serverless endpoint (updated whenever the template or env vars change)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<i32>,

    /// The compute type: "CPU" or "GPU"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_type: Option<String>,

    /// The minimum number of Workers that will always be running
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers_min: Option<i32>,

    /// The maximum number of Workers that can be running at the same time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers_max: Option<i32>,

    /// If true, flash boot is used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flashboot: Option<bool>,

    /// The number of seconds a Worker can run without taking a job before being scaled down
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_timeout: Option<i32>,

    /// The maximum number of ms for an individual request before the Worker is stopped
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_timeout_ms: Option<i32>,

    /// The method used to scale up Workers: "QUEUE_DELAY" or "REQUEST_COUNT"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaler_type: Option<String>,

    /// If scalerType=QUEUE_DELAY, number of seconds before scaling; if REQUEST_COUNT, a divisor for the queue
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaler_value: Option<i32>,

    /// Number of GPUs attached to each Worker
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_count: Option<i32>,

    /// An ordered list of acceptable GPU types (strings)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_type_ids: Option<Vec<String>>,

    /// A list of acceptable CUDA versions for GPU endpoints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_cuda_versions: Option<Vec<String>>,

    /// For CPU endpoints only; a list of CPU instance IDs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_ids: Option<Vec<String>>,

    /// Unique ID of an attached network volume
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_volume_id: Option<String>,

    /// A list of RunPod data center IDs where Workers can be located
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_center_ids: Option<Vec<String>>,

    /// Environment variables for the endpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<std::collections::HashMap<String, String>>,

    /// ID of the template used to create this endpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,

    /// Information about the template (if included in the response)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<Template>,

    /// The UTC timestamp when a Serverless endpoint was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,

    /// Information about the current Workers (if included in the response)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers: Option<Vec<Pod>>,
}

/// A list of endpoints, as returned by the "GET /endpoints" route.
pub type Endpoints = Vec<Endpoint>;

/// Fields required when creating a new endpoint via "POST /endpoints".
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EndpointCreateInput {
    /// According to your OpenAPI, templateId is required
    pub template_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers_max: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers_min: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub flashboot: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_timeout_ms: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_timeout: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaler_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaler_value: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_type_ids: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_count: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_cuda_versions: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_center_ids: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_volume_id: Option<String>,
}

/// Fields for updating an existing endpoint via "PATCH /endpoints/{endpointId}".
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EndpointUpdateInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers_max: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers_min: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub flashboot: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_timeout_ms: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_timeout: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaler_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaler_value: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_type_ids: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_count: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_cuda_versions: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_center_ids: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_volume_id: Option<String>,
}

/// Example "Template" struct, if you want to store the template data from an endpoint:
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: Option<String>,
    pub name: Option<String>,
    // etc.
}

// -----------------------------------------------------------------------------
// 3) Conversion logic for your "CreateOnDemandPodRequest" and "CreateSpotPodRequest"
//    into the REST "PodCreateInput"
// -----------------------------------------------------------------------------
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CreateOnDemandPodRequest {
    pub cloud_type: Option<String>,
    pub gpu_count: Option<i32>, // not directly used by REST
    pub volume_in_gb: Option<i32>,
    pub container_disk_in_gb: Option<i32>,
    pub min_vcpu_count: Option<i32>,
    pub min_memory_in_gb: Option<i32>,
    pub gpu_type_id: Option<String>,
    pub name: Option<String>,
    pub image_name: Option<String>,
    pub docker_args: Option<String>,
    pub ports: Option<String>,
    pub network_volume_id: Option<String>,
    pub volume_mount_path: Option<String>,
    pub env: Vec<EnvVar>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CreateSpotPodRequest {
    pub bid_per_gpu: f64,
    pub cloud_type: Option<String>,
    pub gpu_count: i32,
    pub volume_in_gb: i32,
    pub container_disk_in_gb: i32,
    pub min_vcpu_count: Option<i32>,
    pub min_memory_in_gb: Option<i32>,
    pub gpu_type_id: String,
    pub name: String,
    pub image_name: String,
    pub docker_args: Option<String>,
    pub ports: Option<String>,
    pub network_volume_id: Option<String>,
    pub volume_mount_path: Option<String>,
    pub env: Vec<EnvVar>,
}

impl CreateOnDemandPodRequest {
    pub fn to_pod_create_input(&self, is_spot: bool) -> PodCreateInput {
        let cloud_type = match self.cloud_type.as_deref() {
            Some("SECURE") => "SECURE".to_string(),
            Some("COMMUNITY") => "COMMUNITY".to_string(),
            _ => "SECURE".to_string(),
        };
        let gpu_type_ids = if let Some(gpu_type_id) = &self.gpu_type_id {
            if gpu_type_id.is_empty() {
                vec![]
            } else {
                vec![gpu_type_id.clone()]
            }
        } else {
            vec![]
        };
        let ports: Vec<String> = if let Some(ports_str) = &self.ports {
            ports_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            vec![]
        };
        let env_map: HashMap<String, String> = self
            .env
            .iter()
            .map(|v| (v.key.clone(), v.value.clone()))
            .collect();

        PodCreateInput {
            name: self.name.clone(),
            image_name: self.image_name.clone(),
            container_disk_in_gb: self.container_disk_in_gb,
            volume_in_gb: self.volume_in_gb,
            volume_mount_path: self.volume_mount_path.clone(),
            ports: Some(ports),
            env: Some(env_map),
            docker_start_cmd: None,
            docker_entrypoint: if let Some(args) = &self.docker_args {
                Some(args.split_whitespace().map(|s| s.to_string()).collect())
            } else {
                None
            },
            locked: None,
            container_registry_auth_id: None,
            allowed_cuda_versions: None,
            cloud_type: Some(cloud_type),
            data_center_ids: None,
            gpu_type_ids: Some(gpu_type_ids),
            min_ram_per_gpu: self.min_memory_in_gb,
            min_vcpu_per_gpu: self.min_vcpu_count,
            network_volume_id: self.network_volume_id.clone(),
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
        let cloud_type = match self.cloud_type.as_deref() {
            Some("SECURE") => "SECURE".to_string(),
            Some("COMMUNITY") => "COMMUNITY".to_string(),
            _ => "SECURE".to_string(),
        };
        let gpu_type_ids = if self.gpu_type_id.is_empty() {
            vec![]
        } else {
            vec![self.gpu_type_id.clone()]
        };
        let ports: Vec<String> = if let Some(ports_str) = &self.ports {
            ports_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            vec![]
        };
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
            volume_mount_path: self.volume_mount_path.clone(),
            ports: Some(ports),
            env: Some(env_map),
            docker_start_cmd: if let Some(args) = &self.docker_args {
                Some(args.split_whitespace().map(|s| s.to_string()).collect())
            } else {
                None
            },
            docker_entrypoint: None,
            locked: None,
            container_registry_auth_id: None,
            allowed_cuda_versions: None,
            cloud_type: Some(cloud_type),
            data_center_ids: None,
            gpu_type_ids: Some(gpu_type_ids),
            min_ram_per_gpu: self.min_memory_in_gb,
            min_vcpu_per_gpu: self.min_vcpu_count,
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
