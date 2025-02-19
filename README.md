# runpod.rs

A Rust client library for the RunPod API

## Features

- Create and manage On-Demand GPU pods
- Create and manage Spot (Interruptible) GPU pods
- Start and stop pods
- List and query available GPU types
- Monitor pod status and resources
- Full async support using Tokio

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
runpod-client = "0.1.0"
```

## Quick Start

```rust
use runpod_client::{RunpodClient, CreateOnDemandPodRequest};

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {

    // Initialize client with your API key
    let client = RunpodClient::new("your_api_key_here");

    // List available GPU types
    let gpu_types = client.list_gpu_types().await?;

    // Create an on-demand pod
    let pod_request = CreateOnDemandPodRequest {
        cloud_type: "ALL".to_string(),
        gpu_count: 1,
        gpu_type_id: "NVIDIA RTX A4000".to_string(),
        name: "My Pod".to_string(),
        image_name: "runpod/pytorch:latest".to_string(),
        ..Default::default()
    };

    let pod = client.create_on_demand_pod(pod_request).await?;

    Ok(())
}
```


## Usage

### Managing Pods

```rust
// Create a spot (interruptible) pod
let spot_request = CreateSpotPodRequest {
    bid_per_gpu: 0.5,
    cloud_type: "ALL".to_string(),
    gpu_count: 1,
    gpu_type_id: "NVIDIA RTX A4000".to_string(),
    name: "My Spot Pod".to_string(),
    image_name: "runpod/pytorch:latest".to_string(),
    ..Default::default()
};

let spot_pod = client.create_spot_pod(spot_request).await?;

// List all pods
let pods = client.list_pods().await?;

// Get pod details
let pod_info = client.get_pod("pod_id_here").await?;

// Stop a pod
let stop_result = client.stop_pod("pod_id_here").await?;
```

### GPU Types

```rust
// List all GPU types
let gpu_types = client.list_gpu_types().await?;

// Get specific GPU type details
let gpu_info = client.get_gpu_type("NVIDIA RTX A4000").await?;
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Support

For issues and feature requests, please use the GitHub issues page.