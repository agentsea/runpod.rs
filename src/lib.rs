use reqwest::{Client, Method, Request};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::error::Error;
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

        // Create the headers
        let mut headers = reqwest::header::HeaderMap::new();
        headers.append("Content-Type", "application/json".parse().unwrap());
        headers.append("User-Agent", "Rust-RunPod-Client/1.0".parse().unwrap());

        // Format the request body exactly like the Python implementation
        // Note: Python uses json.dumps({"query": query}) which serializes to a string
        let body_json = json!({"query": query});
        let body_string = serde_json::to_string(&body_json).unwrap();

        // Create the request with proper headers and send as a string
        let request = self
            .http_client
            .post(RUNPOD_GRAPHQL_URL)
            .headers(headers)
            .bearer_auth(&self.api_key)
            .body(body_string); // Send as string, not JSON

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

    /// Delete a Pod by ID.
    pub async fn delete_pod(&self, pod_id: &str) -> Result<(), reqwest::Error> {
        let path = format!("pods/{}", pod_id);
        self.rest_request(Method::DELETE, &path)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// List all datacenters with GPU availability.
    /// If you only need to filter on a single GPU type ID, you can pass it in as a string.
    /// Otherwise, see the "GpuAvailabilityInput" example above.
    /// List all datacenters with all GPU availability.
    pub async fn list_datacenters_with_gpu_availability(
        &self,
    ) -> Result<DatacentersResponseData, reqwest::Error> {
        let query = r#"
        query MyDataCenters {
            myself {
                datacenters {
                    id
                    name
                }
            }
        }
    "#;

        let resp: GraphQLResponse<DatacentersWithGpuAvailResponse> =
            self.graphql_query(query).await?;

        Ok(DatacentersResponseData {
            data: resp.data.map(|d| d.myself.datacenters),
            errors: resp.errors,
        })
    }

    // pub async fn list_available_gpus_with_datacenters(
    //     &self,
    // ) -> Result<GraphQLResponse<Vec<GpuTypeWithDatacenters>>, Box<dyn std::error::Error>> {
    //     // Query GPU types (these fields *are* valid per docs)
    //     let gpu_types_query = r#"
    //     query {
    //       gpuTypes {
    //         id
    //         displayName
    //         memoryInGb
    //         secureCloud
    //         communityCloud
    //         securePrice
    //         communityPrice
    //         communitySpotPrice
    //         secureSpotPrice
    //       }
    //     }
    // "#;

    //     let gpu_types_response = self
    //         .graphql_query::<GraphQLResponse<GpuTypesResponse>>(gpu_types_query)
    //         .await?;

    //     // Then query for datacenters (only fields that are in the docs)
    //     let datacenters_query = r#"
    //     query {
    //       myself {
    //         datacenters {
    //           id
    //           name
    //           location
    //           countryCode
    //           # country is also valid if needed,
    //           # but remove all references to gpuAvailability, etc.
    //         }
    //       }
    //     }
    // "#;

    //     let datacenters_response = self
    //         .graphql_query::<GraphQLResponse<DatacentersResponse>>(datacenters_query)
    //         .await?;

    //     // Extract the data from responses
    //     let gpu_types = match gpu_types_response.data {
    //         Some(data) => data.gpu_types,
    //         None => {
    //             return Err("No GPU types data returned from API".into());
    //         }
    //     };

    //     let datacenters = match datacenters_response.data {
    //         Some(data) => data.myself.datacenters,
    //         None => {
    //             return Err("No datacenter data returned from API".into());
    //         }
    //     };

    //     // Combine the data
    //     let mut result = Vec::new();

    //     for gpu_type in gpu_types {
    //         let available_datacenters = datacenters
    //             .iter()
    //             .filter(|dc| {
    //                 // Use dc.available directly since it's already a boolean
    //                 let has_availability = dc.gpu_availability.iter().any(|ga| ga.available);

    //                 // Then check if any GPU matches the type we're looking for
    //                 let has_matching_gpu = dc.gpu_types.iter().any(|gpu| gpu.id == gpu_type.id);

    //                 // Both conditions must be true
    //                 has_availability && has_matching_gpu
    //             })
    //             .map(|dc| DatacenterInfo {
    //                 id: dc.id.clone(),
    //                 name: dc.name.clone(),
    //                 available: dc.available,
    //                 country_name: "".to_string(),
    //             })
    //             .collect();

    //         result.push(GpuTypeWithDatacenters {
    //             id: gpu_type.id.clone(),
    //             display_name: gpu_type.display_name.clone(),
    //             memory_in_gb: gpu_type.memory_in_gb,
    //             secure_cloud: gpu_type.secure_cloud,
    //             community_cloud: gpu_type.community_cloud,
    //             datacenters: available_datacenters,
    //             secure_price: gpu_type.secure_price,
    //             community_price: gpu_type.community_price,
    //             community_spot_price: gpu_type.community_spot_price,
    //             secure_spot_price: gpu_type.secure_spot_price,
    //         });
    //     }

    //     Ok(GraphQLResponse {
    //         data: Some(result),
    //         errors: None,
    //     })
    // }

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
        // Build the initial JSON
        let mut payload = serde_json::json!({
            "cloudType": req.cloud_type.unwrap_or("SECURE".to_string()),
            "computeType": "GPU",
            "gpuCount": req.gpu_count.unwrap_or(1),
            "volumeInGb": req.volume_in_gb.unwrap_or(20),
            "containerDiskInGb": req.container_disk_in_gb.unwrap_or(50),
            "minVCPUPerGPU": req.min_vcpu_count.unwrap_or(2),
            "minRAMPerGPU": req.min_memory_in_gb.unwrap_or(8),
            "gpuTypeIds": [req.gpu_type_id.unwrap_or_default()],
            "name": req.name.unwrap_or_default(),
            "imageName": req.image_name.unwrap_or_default(),
            "dockerEntrypoint": req.docker_entrypoint.unwrap_or_default(),
            "supportPublicIp": true,
            "ports": req.ports,
            "env": req.env
                .into_iter()
                .map(|e| (e.key, e.value))
                .collect::<std::collections::HashMap<_, _>>(),
        });

        // If user provided a volumeMountPath, insert it; otherwise don't.
        if let Some(volume_mount_path) = req.volume_mount_path {
            payload.as_object_mut().unwrap().insert(
                "volumeMountPath".to_string(),
                serde_json::Value::String(volume_mount_path),
            );
        }

        if let Some(network_volume_id) = req.network_volume_id {
            payload.as_object_mut().unwrap().insert(
                "networkVolumeId".to_string(),
                serde_json::Value::String(network_volume_id),
            );
        }

        // Then do the request + status check, same as before...
        let response = self
            .http_client
            .post("https://rest.runpod.io/v1/pods")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&payload)
            .send()
            .await?;

        // ...and the rest is unchanged...
        if let Err(status_err) = response.error_for_status_ref() {
            let err_text = response.text().await.unwrap_or_else(|_| "".to_string());
            eprintln!("RunPod create_on_demand_pod error body: {}", err_text);
            return Err(status_err);
        }

        let pod = response.json::<Pod>().await?;
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
                    cost_per_hr: p.cost_per_hr,
                    adjusted_cost_per_hr: p.adjusted_cost_per_hr,
                    interruptible: p.interruptible,
                    ports: p.ports.clone(),
                    public_ip: p.public_ip.clone(),
                    image: p.image.unwrap_or_default(),
                    env: p.env.clone(),
                    last_started_at: p.last_started_at.clone(),
                    network_volume: p.network_volume.map(|nv| NetworkVolume {
                        id: nv.id,
                        name: nv.name,
                        size: nv.size,
                        data_center_id: nv.data_center_id,
                    }),
                    machine: p.machine.map(|m| PodMachineInfo {
                        id: m.id.unwrap_or_default(),
                        location: m.location.unwrap_or_default(),
                        support_public_ip: m.support_public_ip.unwrap_or_default(),
                        secure_cloud: m.secure_cloud.unwrap_or_default(),
                    }),
                    docker_entrypoint: p.docker_entrypoint,
                    docker_start_cmd: p.docker_start_cmd,
                })
                .collect(),
        };
        Ok(PodsListResponseData {
            data: Some(my_pods),
            errors: None,
        })
    }

    pub async fn get_container_logs(&self, container_id: &str) -> Result<String, reqwest::Error> {
        let endpoint = format!("https://hapi.runpod.net/containers/{container_id}/logs");
        let container_logs: ContainerLogs = self
            .http_client
            .get(&endpoint)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?; // <= This automatically yields a `reqwest::Error` if JSON parsing fails.

        Ok(container_logs.container.join("\n"))
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
                cost_per_hr: pod.cost_per_hr,
                adjusted_cost_per_hr: pod.adjusted_cost_per_hr,
                interruptible: pod.interruptible,
                ports: pod.ports.clone(),
                public_ip: pod.public_ip.clone(),
                image: pod.image.unwrap_or_default(),
                env: pod.env.clone(),
                last_started_at: pod.last_started_at.clone(),
                network_volume: pod.network_volume.map(|nv| NetworkVolume {
                    id: nv.id,
                    name: nv.name,
                    size: nv.size,
                    data_center_id: nv.data_center_id,
                }),
                machine: pod.machine.map(|m| PodMachineInfo {
                    id: m.id.unwrap_or_default(),
                    location: m.location.unwrap_or_default(),
                    support_public_ip: m.support_public_ip.unwrap_or_default(),
                    secure_cloud: m.secure_cloud.unwrap_or_default(),
                    // Add other machine fields as needed
                }),
                docker_entrypoint: pod.docker_entrypoint,
                docker_start_cmd: pod.docker_start_cmd,
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
#[derive(Debug, Serialize, Deserialize)]
pub struct GraphQLError {
    pub message: String,
    pub locations: Option<Vec<serde_json::Value>>,
    pub path: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GraphQLResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GPUTypesListResponse {
    #[serde(rename = "gpuTypes")]
    pub gpu_types: Vec<GpuTypeMinimal>,
}

#[derive(Debug, Deserialize)]
struct GpuTypesResponse {
    #[serde(rename = "gpuTypes")]
    gpu_types: Vec<GpuType>,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuType {
    pub id: String,
    pub display_name: String,
    pub memory_in_gb: f64,
    pub secure_cloud: bool,
    pub community_cloud: bool,
    pub secure_price: Option<f64>,
    pub community_price: Option<f64>,
    pub community_spot_price: Option<f64>,
    pub secure_spot_price: Option<f64>,
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

#[derive(Debug, Deserialize, Clone)]
pub struct DatacenterWithGpuTypes {
    pub id: String,
    pub name: String,
    pub location: String,
    pub available: bool,
    #[serde(rename = "gpuTypes")]
    pub gpu_types: Vec<DatacenterGpuType>,
}

#[derive(Debug, Deserialize, Clone)]
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

#[derive(serde::Deserialize)]
struct DatacentersResponse {
    myself: MyselfDatacenters,
}

#[derive(serde::Deserialize)]
struct MyselfDatacenters {
    datacenters: Vec<DatacenterQ>,
}

#[derive(Debug, Deserialize)]
struct ContainerLogs {
    container: Vec<String>,
}

#[derive(serde::Deserialize)]
struct DatacenterQ {
    id: String,
    name: String,
    location: String,
    #[serde(rename = "countryCode")]
    country_code: String,
}

/// The top-level data structure for this particular `myself.datacenters` response.
#[derive(Debug, serde::Deserialize)]
pub struct DatacentersWithGpuAvailResponse {
    pub myself: MyselfDataCenters,
}

/// Just holds the array of datacenters.
#[derive(Debug, serde::Deserialize)]
pub struct MyselfDataCenters {
    pub datacenters: Vec<DataCenter>,
}

/// Represents a single DataCenter, including its GPU availability.
#[derive(Debug, serde::Deserialize)]
pub struct DataCenter {
    pub id: String,
    pub name: String,
    pub location: String,
    pub countryCode: String,
    pub gpuAvailability: Vec<GpuAvailability>,
}

/// Holds GPU availability for a particular GPU type.
#[derive(Debug, serde::Deserialize)]
pub struct GpuAvailability {
    pub gpuTypeId: String,
    pub count: i32,
}

/// A convenience struct for returning data from the function,
/// similar to your existing style of "ResponseData".
#[derive(Debug)]
pub struct DatacentersResponseData {
    pub data: Option<Vec<DataCenter>>,
    pub errors: Option<Vec<GraphQLError>>,
}

// -----------------------------------------------------------------------------
// REST Data Structures
// -----------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct GpuTypeWithDatacenters {
    pub id: String,
    pub display_name: String,
    pub memory_in_gb: f64,
    pub secure_cloud: bool,
    pub community_cloud: bool,
    pub secure_price: Option<f64>,
    pub community_price: Option<f64>,
    pub community_spot_price: Option<f64>,
    pub secure_spot_price: Option<f64>,
    pub datacenters: Vec<DatacenterInfo>,
}

#[derive(Debug, Deserialize)]
pub struct DatacenterInfo {
    pub id: String,
    pub name: String,
    pub available: bool,
    pub country_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Datacenter {
    pub id: String,
    pub name: String,
    pub available: bool,
    pub location: Location,
    #[serde(rename = "gpuTypes")]
    pub gpu_types: Vec<GpuType>,
}

#[derive(Debug, Deserialize)]
pub struct Location {
    #[serde(rename = "countryName")]
    pub country_name: String,
}

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
    #[serde(default)]
    pub desired_status: String,
    #[serde(default)]
    pub last_status_change: String,
    #[serde(default)]
    pub machine_id: String,
    #[serde(default)]
    pub memory_in_gb: f64,
    #[serde(default)]
    pub vcpu_count: f64,
    #[serde(default)]
    pub container_disk_in_gb: i32,
    #[serde(default)]
    pub volume_in_gb: f64,
    #[serde(default)]
    pub volume_mount_path: Option<String>,
    #[serde(default)]
    pub volume_encrypted: bool,
    #[serde(default)]
    pub cost_per_hr: f64,
    #[serde(default)]
    pub adjusted_cost_per_hr: f64,
    #[serde(default)]
    pub interruptible: bool,
    #[serde(default)]
    pub ports: Vec<String>,
    #[serde(default)]
    pub public_ip: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub last_started_at: Option<String>,
    #[serde(default)]
    pub gpu: Option<GpuInfo>,
    #[serde(default)]
    pub machine: Option<MachineInfo>,
    #[serde(default)]
    pub runtime: Option<RuntimeInfo>,
    #[serde(default)]
    pub network_volume: Option<NetworkVolumeInfo>,
    #[serde(default)]
    pub savings_plans: Option<Vec<SavingsPlan>>,
    #[serde(default)]
    pub docker_entrypoint: Option<Vec<String>>,
    #[serde(default)]
    pub docker_start_cmd: Option<Vec<String>>,
    // Add other fields as needed
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

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub id: String,
    pub count: i32,
    pub display_name: String,
    // Add other GPU fields as needed
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(default)]
pub struct MachineInfo {
    // No references to gpuDisplayName
    #[serde(rename = "gpuTypeId")]
    pub gpu_type_id: Option<String>,

    pub id: Option<String>,
    pub location: Option<String>,
    pub support_public_ip: Option<bool>,
    pub secure_cloud: Option<bool>,
    // etc.
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
    pub docker_entrypoint: Option<Vec<String>>,
    pub docker_start_cmd: Option<Vec<String>>,
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

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct PodMachineInfo {
    // Mark everything optional or provide defaults so Serde won’t error out:
    #[serde(default)]
    pub id: String,

    #[serde(default)]
    pub location: String,

    #[serde(default)]
    pub support_public_ip: bool,

    #[serde(default)]
    pub secure_cloud: bool,
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
#[serde(rename_all = "camelCase")]
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
    pub docker_args: Option<Vec<String>>,
    pub docker_entrypoint: Option<Vec<String>>,
    pub ports: Option<String>,
    pub network_volume_id: Option<String>,
    pub volume_mount_path: Option<String>,
    pub env: Vec<EnvVar>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
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
    pub docker_entrypoint: Option<Vec<String>>,
    pub docker_args: Option<Vec<String>>,
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
            docker_entrypoint: self.docker_entrypoint.clone(),
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
            docker_start_cmd: self.docker_args.clone(),
            docker_entrypoint: self.docker_entrypoint.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    #[ignore] // This test is ignored by default since it requires an API key
    async fn integration_test_list_gpu_types() {
        // Get API key from environment variable
        let api_key =
            env::var("RUNPOD_API_KEY").expect("RUNPOD_API_KEY environment variable must be set");

        // Create a real client
        let client = RunpodClient::new(api_key);

        // Make the actual API call
        let result = client
            .list_gpu_types_graphql()
            .await
            .expect("API call failed");

        // Print the result for inspection
        println!("GPU Types: {:#?}", result);

        // Basic validation that we got some data
        assert!(result.errors.is_none());
        if let Some(gpu_types) = result.data {
            assert!(!gpu_types.is_empty(), "Expected at least one GPU type");

            // Print each GPU type
            for gpu in &gpu_types {
                println!(
                    "- {} ({}): {}GB",
                    gpu.display_name,
                    gpu.id,
                    gpu.memory_in_gb.unwrap_or(0)
                );
            }
        }
    }

    #[tokio::test]
    #[ignore] // This test is ignored by default since it requires an API key
    async fn integration_test_create_on_demand_pod() {
        // Get API key from environment variable
        let api_key =
            env::var("RUNPOD_API_KEY").expect("RUNPOD_API_KEY environment variable must be set");

        // Create a real client
        let client = RunpodClient::new(api_key);

        // Create environment variables for the pod
        let env_vec = vec![EnvVar {
            key: "TEST_VAR".to_string(),
            value: "test_value".to_string(),
        }];

        // Create the request
        let request = CreateOnDemandPodRequest {
            cloud_type: Some("SECURE".to_string()),
            gpu_count: Some(1),
            volume_in_gb: Some(20),
            container_disk_in_gb: Some(50),
            min_vcpu_count: Some(4),
            min_memory_in_gb: Some(16),
            gpu_type_id: Some("NVIDIA A100-SXM4-80GB".to_string()),
            name: Some("test_pod_from_rust".to_string()),
            image_name: Some(
                "runpod/pytorch:2.1.0-py3.10-cuda11.8.0-devel-ubuntu22.04".to_string(),
            ),
            docker_args: None,
            docker_entrypoint: Some(vec![
                "sh".to_string(),
                "-c".to_string(),
                "while true; do echo \"Hello from Docker Entrypoint!\"; sleep 5; done".to_string(),
            ]),
            ports: Some("8888".to_string()),
            volume_mount_path: None,
            // volume_mount_path: Some("/workspace".to_string()),
            env: env_vec,
            network_volume_id: None,
        };

        // Make the actual API call
        let result = client.create_on_demand_pod(request).await;

        // Print any error details for debugging
        if let Err(ref e) = result {
            println!("Error creating pod: {:?}", e);
        }

        // Verify the result
        let pod_data = result.expect("Failed to create pod");

        // Print the pod ID for reference
        if let Some(data) = pod_data.data {
            println!("Successfully created pod with ID: {}", data.id);

            // Get the pod details to verify it was created correctly
            match client.get_pod(&data.id).await {
                Ok(pod_info) => {
                    println!("Pod details: {:?}", pod_info);
                }
                Err(e) => {
                    println!("Error getting pod details: {:?}", e);
                }
            }
            // Sleep for a few seconds to allow the pod to initialize
            tokio::time::sleep(tokio::time::Duration::from_secs(500000)).await;

            // Now delete the pod
            client
                .delete_pod(&data.id)
                .await
                .expect("Failed to delete the pod");
            println!("Deleted pod with ID: {}", data.id);
        } else {
            panic!("No pod data returned");
        }
    }

    // #[tokio::test]
    // #[ignore] // This test is ignored by default since it requires an API key
    // async fn integration_test_list_datacenters_with_gpu_availability() {
    //     let api_key =
    //         env::var("RUNPOD_API_KEY").expect("RUNPOD_API_KEY environment variable must be set");

    //     let client = RunpodClient::new(api_key);

    //     // Notice we are not passing an ID or input.
    //     let result = client
    //         .list_datacenters_with_gpu_availability()
    //         .await
    //         .expect("API call failed");

    //     println!("Datacenters with GPU Availability: {:#?}", result);

    //     // Basic validation
    //     assert!(result.errors.is_none());
    //     if let Some(datacenters) = result.data {
    //         assert!(!datacenters.is_empty(), "Expected at least one datacenter");

    //         for dc in &datacenters {
    //             println!(
    //                 "- {} ({}), located in {} / {}",
    //                 dc.name, dc.id, dc.location, dc.countryCode
    //             );
    //             for availability in &dc.gpuAvailability {
    //                 println!(
    //                     "  ● GPU Type {} has {} available",
    //                     availability.gpuTypeId, availability.count
    //                 );
    //             }
    //         }
    //     }
    // }
    // #[tokio::test]
    // #[ignore] // This test is ignored by default since it requires an API key
    // async fn integration_test_list_available_gpus_with_datacenters() {
    //     // Get API key from environment variable
    //     let api_key =
    //         env::var("RUNPOD_API_KEY").expect("RUNPOD_API_KEY environment variable must be set");

    //     // Create a real client
    //     let client = RunpodClient::new(api_key);

    //     // Make the actual API call
    //     let result = client.list_available_gpus_with_datacenters().await;

    //     // Check if there was an error and print more details
    //     if let Err(err) = &result {
    //         println!("Error details: {:?}", err);
    //         if let Some(reqwest_err) = err.downcast_ref::<reqwest::Error>() {
    //             if let Some(status) = reqwest_err.status() {
    //                 println!("HTTP Status: {}", status);
    //             }
    //         }
    //         // The correct way to get the response text from a reqwest::Error
    //         if let Some(source) = err.source() {
    //             println!("Error source: {}", source);
    //         }
    //     }

    //     // Now unwrap the result
    //     let result = result.expect("API call failed");

    //     // Print the result for inspection
    //     println!("GPU Types with Datacenters: {:#?}", result);

    //     // Basic validation that we got some data
    //     assert!(
    //         result.errors.is_none(),
    //         "GraphQL errors: {:?}",
    //         result.errors
    //     );
    //     if let Some(gpu_types) = result.data {
    //         assert!(!gpu_types.is_empty(), "Expected at least one GPU type");

    //         // Print each GPU type and its datacenters
    //         for gpu_type in &gpu_types {
    //             println!(
    //                 "GPU: {} ({}) - Memory: {}GB - Secure: {} - Community: {}",
    //                 gpu_type.display_name,
    //                 gpu_type.id,
    //                 gpu_type.memory_in_gb,
    //                 gpu_type.secure_cloud,
    //                 gpu_type.community_cloud
    //             );

    //             // Print pricing information if available
    //             if gpu_type.secure_price.is_some()
    //                 || gpu_type.community_price.is_some()
    //                 || gpu_type.secure_spot_price.is_some()
    //                 || gpu_type.community_spot_price.is_some()
    //             {
    //                 println!("  Pricing:");
    //                 if let Some(price) = gpu_type.secure_price {
    //                     println!("    Secure: ${:.2}", price);
    //                 }
    //                 if let Some(price) = gpu_type.community_price {
    //                     println!("    Community: ${:.2}", price);
    //                 }
    //                 if let Some(price) = gpu_type.secure_spot_price {
    //                     println!("    Secure Spot: ${:.2}", price);
    //                 }
    //                 if let Some(price) = gpu_type.community_spot_price {
    //                     println!("    Community Spot: ${:.2}", price);
    //                 }
    //             }

    //             if !gpu_type.datacenters.is_empty() {
    //                 println!("  Available in datacenters:");
    //                 for datacenter in &gpu_type.datacenters {
    //                     println!(
    //                         "    - {} ({}) - Location: {} - Available: {}",
    //                         datacenter.name,
    //                         datacenter.id,
    //                         datacenter.country_name,
    //                         datacenter.available
    //                     );
    //                 }
    //             } else {
    //                 println!("  Not available in any datacenters");
    //             }
    //             println!();
    //         }
    //     }
    // }
}
