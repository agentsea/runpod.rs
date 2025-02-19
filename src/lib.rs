use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

const RUNPOD_ENDPOINT: &str = "https://api.runpod.io/graphql";

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

    /// Low-level function to execute a GraphQL query or mutation.
    /// You normally wouldn't call this directly; instead, use helper methods.
    async fn graphql_query<T: for<'de> Deserialize<'de>>(
        &self,
        graphql_body: &serde_json::Value,
    ) -> Result<T, reqwest::Error> {
        let url = format!("{}?api_key={}", RUNPOD_ENDPOINT, self.api_key);

        let response = self
            .http_client
            .post(&url)
            .json(graphql_body)
            .send()
            .await?
            .error_for_status()?
            .json::<T>()
            .await?;

        Ok(response)
    }

    // ---------------------------------------------------------------------
    // 1) Create an On-Demand Pod
    // ---------------------------------------------------------------------
    pub async fn create_on_demand_pod(
        &self,
        req: CreateOnDemandPodRequest,
    ) -> Result<PodCreateResponseData, reqwest::Error> {
        // Build the GraphQL mutation string.
        let query = format!(
            r#"
            mutation {{
                podFindAndDeployOnDemand(input: {{
                    cloudType: {cloud_type},
                    gpuCount: {gpu_count},
                    volumeInGb: {volume_in_gb},
                    containerDiskInGb: {container_disk_in_gb},
                    minVcpuCount: {min_vcpu_count},
                    minMemoryInGb: {min_memory_in_gb},
                    gpuTypeId: "{gpu_type_id}",
                    name: "{name}",
                    imageName: "{image_name}",
                    dockerArgs: "{docker_args}",
                    ports: "{ports}",
                    volumeMountPath: "{volume_mount_path}",
                    env: [{env}]
                }}) {{
                    id
                    imageName
                    env {{ key value }}
                    machineId
                    machine {{ podHostId }}
                }}
            }}
            "#,
            cloud_type = req.cloud_type,
            gpu_count = req.gpu_count,
            volume_in_gb = req.volume_in_gb,
            container_disk_in_gb = req.container_disk_in_gb,
            min_vcpu_count = req.min_vcpu_count,
            min_memory_in_gb = req.min_memory_in_gb,
            gpu_type_id = req.gpu_type_id,
            name = req.name,
            image_name = req.image_name,
            docker_args = req.docker_args,
            ports = req.ports,
            volume_mount_path = req.volume_mount_path,
            env = env_to_string(&req.env),
        );

        let body = json!({ "query": query });

        // Execute the request
        let resp: GraphQLResponse<PodCreateResponse> = self.graphql_query(&body).await?;
        Ok(PodCreateResponseData {
            data: resp.data.map(|d| d.pod_find_and_deploy_on_demand),
            errors: resp.errors,
        })
    }

    // ---------------------------------------------------------------------
    // 2) Create a Spot (Interruptible) Pod
    // ---------------------------------------------------------------------
    pub async fn create_spot_pod(
        &self,
        req: CreateSpotPodRequest,
    ) -> Result<PodCreateResponseData, reqwest::Error> {
        let query = format!(
            r#"
            mutation {{
                podRentInterruptable(input: {{
                    bidPerGpu: {bid_per_gpu},
                    cloudType: {cloud_type},
                    gpuCount: {gpu_count},
                    volumeInGb: {volume_in_gb},
                    containerDiskInGb: {container_disk_in_gb},
                    minVcpuCount: {min_vcpu_count},
                    minMemoryInGb: {min_memory_in_gb},
                    gpuTypeId: "{gpu_type_id}",
                    name: "{name}",
                    imageName: "{image_name}",
                    dockerArgs: "{docker_args}",
                    ports: "{ports}",
                    volumeMountPath: "{volume_mount_path}",
                    env: [{env}]
                }}) {{
                    id
                    imageName
                    env {{ key value }}
                    machineId
                    machine {{ podHostId }}
                }}
            }}
            "#,
            bid_per_gpu = req.bid_per_gpu,
            cloud_type = req.cloud_type,
            gpu_count = req.gpu_count,
            volume_in_gb = req.volume_in_gb,
            container_disk_in_gb = req.container_disk_in_gb,
            min_vcpu_count = req.min_vcpu_count,
            min_memory_in_gb = req.min_memory_in_gb,
            gpu_type_id = req.gpu_type_id,
            name = req.name,
            image_name = req.image_name,
            docker_args = req.docker_args,
            ports = req.ports,
            volume_mount_path = req.volume_mount_path,
            env = env_to_string(&req.env),
        );

        let body = json!({ "query": query });

        let resp: GraphQLResponse<PodCreateResponse> = self.graphql_query(&body).await?;
        Ok(PodCreateResponseData {
            data: resp.data.map(|d| d.pod_rent_interruptable),
            errors: resp.errors,
        })
    }

    // ---------------------------------------------------------------------
    // 3) Start (Resume) a Pod (On-Demand)
    // ---------------------------------------------------------------------
    pub async fn start_on_demand_pod(
        &self,
        pod_id: &str,
        gpu_count: i32,
    ) -> Result<PodStartResponseData, reqwest::Error> {
        let query = format!(
            r#"
            mutation {{
                podResume(input: {{
                    podId: "{pod_id}",
                    gpuCount: {gpu_count}
                }}) {{
                    id
                    desiredStatus
                    imageName
                    env {{ key value }}
                    machineId
                    machine {{ podHostId }}
                }}
            }}
            "#,
            pod_id = pod_id,
            gpu_count = gpu_count
        );
        let body = json!({ "query": query });

        let resp: GraphQLResponse<PodStartResponse> = self.graphql_query(&body).await?;
        Ok(PodStartResponseData {
            data: resp.data.and_then(|d| d.pod_resume),
            errors: resp.errors,
        })
    }

    // ---------------------------------------------------------------------
    // 4) Start (Resume) a Pod (Spot)
    // ---------------------------------------------------------------------
    pub async fn start_spot_pod(
        &self,
        pod_id: &str,
        bid_per_gpu: f64,
        gpu_count: i32,
    ) -> Result<PodStartResponseData, reqwest::Error> {
        let query = format!(
            r#"
            mutation {{
                podBidResume(input: {{
                    podId: "{pod_id}",
                    bidPerGpu: {bid_per_gpu},
                    gpuCount: {gpu_count}
                }}) {{
                    id
                    desiredStatus
                    imageName
                    env {{ key value }}
                    machineId
                    machine {{ podHostId }}
                }}
            }}
            "#,
            pod_id = pod_id,
            bid_per_gpu = bid_per_gpu,
            gpu_count = gpu_count
        );
        let body = json!({ "query": query });

        let resp: GraphQLResponse<PodStartResponse> = self.graphql_query(&body).await?;
        Ok(PodStartResponseData {
            data: resp.data.and_then(|d| d.pod_bid_resume),
            errors: resp.errors,
        })
    }

    // ---------------------------------------------------------------------
    // 5) Stop a Pod
    // ---------------------------------------------------------------------
    pub async fn stop_pod(&self, pod_id: &str) -> Result<PodStopResponseData, reqwest::Error> {
        let query = format!(
            r#"
            mutation {{
                podStop(input: {{
                    podId: "{pod_id}"
                }}) {{
                    id
                    desiredStatus
                }}
            }}
            "#,
            pod_id = pod_id
        );
        let body = json!({ "query": query });

        let resp: GraphQLResponse<PodStopResponse> = self.graphql_query(&body).await?;
        Ok(PodStopResponseData {
            data: resp.data.map(|d| d.pod_stop),
            errors: resp.errors,
        })
    }

    // ---------------------------------------------------------------------
    // 6) List all Pods
    // ---------------------------------------------------------------------
    pub async fn list_pods(&self) -> Result<PodsListResponseData, reqwest::Error> {
        let query = r#"
            query Pods {
                myself {
                    pods {
                        id
                        name
                        runtime {
                            uptimeInSeconds
                            ports {
                                ip
                                isIpPublic
                                privatePort
                                publicPort
                                type
                            }
                            gpus {
                                id
                                gpuUtilPercent
                                memoryUtilPercent
                            }
                            container {
                                cpuPercent
                                memoryPercent
                            }
                        }
                    }
                }
            }
        "#;
        let body = json!({ "query": query });

        let resp: GraphQLResponse<PodsListResponse> = self.graphql_query(&body).await?;
        Ok(PodsListResponseData {
            data: resp.data.map(|d| d.myself),
            errors: resp.errors,
        })
    }

    // ---------------------------------------------------------------------
    // 7) Get Pod by ID
    // ---------------------------------------------------------------------
    pub async fn get_pod(&self, pod_id: &str) -> Result<PodInfoResponseData, reqwest::Error> {
        let query = format!(
            r#"
            query Pod {{
                pod(input: {{
                    podId: "{pod_id}"
                }}) {{
                    id
                    name
                    runtime {{
                        uptimeInSeconds
                        ports {{
                            ip
                            isIpPublic
                            privatePort
                            publicPort
                            type
                        }}
                        gpus {{
                            id
                            gpuUtilPercent
                            memoryUtilPercent
                        }}
                        container {{
                            cpuPercent
                            memoryPercent
                        }}
                    }}
                }}
            }}
            "#,
            pod_id = pod_id
        );
        let body = json!({ "query": query });

        let resp: GraphQLResponse<PodInfoResponse> = self.graphql_query(&body).await?;
        Ok(PodInfoResponseData {
            data: resp.data.map(|d| d.pod),
            errors: resp.errors,
        })
    }

    // ---------------------------------------------------------------------
    // 8) List GPU Types
    // ---------------------------------------------------------------------
    pub async fn list_gpu_types(&self) -> Result<GPUTypesListResponseData, reqwest::Error> {
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

    // ---------------------------------------------------------------------
    // 9) Get GPU Type by ID
    // ---------------------------------------------------------------------
    pub async fn get_gpu_type(
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
}

// ---------------------------------------------------------------------
// Helper to turn env Vec<EnvVar> into a GraphQL list string
// ---------------------------------------------------------------------
fn env_to_string(env: &[EnvVar]) -> String {
    env.iter()
        .map(|env_var| {
            format!(
                r#"{{ key: "{}", value: "{}" }}"#,
                env_var.key, env_var.value
            )
        })
        .collect::<Vec<String>>()
        .join(", ")
}

// ---------------------------------------------------------------------
// GraphQL request/response data structures
// ---------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GraphQLResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GraphQLError {
    pub message: String,
    // You can add more fields if needed (e.g., locations, etc.)
}

// ---------------------------------------------------------------------
// 1) Create On-Demand Pod
// ---------------------------------------------------------------------
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CreateOnDemandPodRequest {
    pub cloud_type: String,        // e.g. "ALL"
    pub gpu_count: i32,            // e.g. 1
    pub volume_in_gb: i32,         // e.g. 40
    pub container_disk_in_gb: i32, // e.g. 40
    pub min_vcpu_count: i32,       // e.g. 2
    pub min_memory_in_gb: i32,     // e.g. 15
    pub gpu_type_id: String,       // e.g. "NVIDIA RTX A6000"
    pub name: String,              // e.g. "RunPod Tensorflow"
    pub image_name: String,        // e.g. "runpod/tensorflow"
    pub docker_args: String,       // e.g. ""
    pub ports: String,             // e.g. "8888/http"
    pub volume_mount_path: String, // e.g. "/workspace"
    pub env: Vec<EnvVar>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodCreateResponse {
    #[serde(rename = "podFindAndDeployOnDemand")]
    pub pod_find_and_deploy_on_demand: PodInfoMinimal,
    #[serde(rename = "podRentInterruptable", default)]
    pub pod_rent_interruptable: PodInfoMinimal,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodCreateResponseData {
    pub data: Option<PodInfoMinimal>, // Because we can get either from OnDemand or Spot creation
    pub errors: Option<Vec<GraphQLError>>,
}

// ---------------------------------------------------------------------
// 2) Create Spot Pod
// ---------------------------------------------------------------------
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CreateSpotPodRequest {
    pub bid_per_gpu: f64,          // e.g. 0.2
    pub cloud_type: String,        // e.g. "SECURE"
    pub gpu_count: i32,            // e.g. 1
    pub volume_in_gb: i32,         // e.g. 40
    pub container_disk_in_gb: i32, // e.g. 40
    pub min_vcpu_count: i32,       // e.g. 2
    pub min_memory_in_gb: i32,     // e.g. 15
    pub gpu_type_id: String,       // e.g. "NVIDIA RTX A6000"
    pub name: String,              // e.g. "RunPod Pytorch"
    pub image_name: String,        // e.g. "runpod/pytorch"
    pub docker_args: String,       // e.g. ""
    pub ports: String,             // e.g. "8888/http"
    pub volume_mount_path: String, // e.g. "/workspace"
    pub env: Vec<EnvVar>,
}

// ---------------------------------------------------------------------
// 3) 4) Start Pod response
// ---------------------------------------------------------------------
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodStartResponse {
    #[serde(rename = "podResume", default)]
    pub pod_resume: Option<PodInfoMinimal>,

    #[serde(rename = "podBidResume", default)]
    pub pod_bid_resume: Option<PodInfoMinimal>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodStartResponseData {
    pub data: Option<PodInfoMinimal>,
    pub errors: Option<Vec<GraphQLError>>,
}

// ---------------------------------------------------------------------
// 5) Stop Pod
// ---------------------------------------------------------------------
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodStopResponse {
    #[serde(rename = "podStop")]
    pub pod_stop: PodInfoMinimalStop,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodStopResponseData {
    pub data: Option<PodInfoMinimalStop>,
    pub errors: Option<Vec<GraphQLError>>,
}

// ---------------------------------------------------------------------
// 6) List Pods
// ---------------------------------------------------------------------
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodsListResponse {
    pub myself: MyselfPods,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct MyselfPods {
    pub pods: Vec<PodInfoFull>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodsListResponseData {
    pub data: Option<MyselfPods>,
    pub errors: Option<Vec<GraphQLError>>,
}

// ---------------------------------------------------------------------
// 7) Get Pod by ID
// ---------------------------------------------------------------------
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodInfoResponse {
    pub pod: PodInfoFull,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PodInfoResponseData {
    pub data: Option<PodInfoFull>,
    pub errors: Option<Vec<GraphQLError>>,
}

// ---------------------------------------------------------------------
// 8) List GPU Types
// ---------------------------------------------------------------------
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

// ---------------------------------------------------------------------
// 9) Get GPU Type by ID
// ---------------------------------------------------------------------
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GPUTypesExtendedResponse {
    #[serde(rename = "gpuTypes")]
    pub gpu_types: Vec<GpuTypeExtended>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GPUTypeResponseData {
    pub data: Option<Vec<GpuTypeExtended>>,
    pub errors: Option<Vec<GraphQLError>>,
}

// ---------------------------------------------------------------------
// Common Data Structures
// ---------------------------------------------------------------------
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
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct MachineHost {
    #[serde(rename = "podHostId")]
    pub pod_host_id: Option<String>,
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
    pub ports: Option<Vec<PortInfo>>,
    pub gpus: Option<Vec<GpuInfo>>,
    pub container: Option<ContainerInfo>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PortInfo {
    pub ip: Option<String>,
    #[serde(rename = "isIpPublic")]
    pub is_ip_public: Option<bool>,
    #[serde(rename = "privatePort")]
    pub private_port: Option<i32>,
    #[serde(rename = "publicPort")]
    pub public_port: Option<i32>,
    #[serde(rename = "type")]
    pub port_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GpuInfo {
    pub id: Option<String>,
    #[serde(rename = "gpuUtilPercent")]
    pub gpu_util_percent: Option<f64>,
    #[serde(rename = "memoryUtilPercent")]
    pub memory_util_percent: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ContainerInfo {
    #[serde(rename = "cpuPercent")]
    pub cpu_percent: Option<f64>,
    #[serde(rename = "memoryPercent")]
    pub memory_percent: Option<f64>,
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
