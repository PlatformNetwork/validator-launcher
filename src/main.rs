// SPDX-FileCopyrightText: © 2024-2025 Phala Network <dstack@phala.network>
//
// SPDX-License-Identifier: Apache-2.0

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::time::{sleep, timeout};
use tracing::{error, info, warn};
use x25519_dalek::{EphemeralSecret, PublicKey};

mod config_tui;

const API_URL: &str = "https://api.platform.network/config/compose/validator_vm";
const POLL_INTERVAL: Duration = Duration::from_secs(5);
const VM_KILL_TIMEOUT: Duration = Duration::from_secs(60);
const VM_NAME: &str = "validator_vm";
pub const PLATFORM_CONFIG_PATH: &str = "/etc/platform-validator/config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ComposeConfig {
    vm_type: String,
    compose_content: String,
    #[serde(default)]
    description: Option<String>,
    updated_at: String,
    #[serde(default)]
    required_env: Vec<String>,
    #[serde(default)]
    provisioning: VmProvisioningConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VmProvisioningConfig {
    #[serde(default)]
    env_keys: Vec<String>,
    #[serde(default)]
    manifest_defaults: ManifestDefaults,
    #[serde(default)]
    vm_parameters: VmParameters,
}

impl Default for VmProvisioningConfig {
    fn default() -> Self {
        Self {
            env_keys: Vec::new(),
            manifest_defaults: ManifestDefaults::default(),
            vm_parameters: VmParameters::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestDefaults {
    manifest_version: u32,
    #[serde(default)]
    name: Option<String>,
    runner: String,
    #[serde(default)]
    kms_enabled: bool,
    #[serde(default)]
    gateway_enabled: bool,
    #[serde(default)]
    local_key_provider_enabled: bool,
    #[serde(default)]
    key_provider_id: String,
    #[serde(default)]
    public_logs: bool,
    #[serde(default)]
    public_sysinfo: bool,
    #[serde(default)]
    public_tcbinfo: bool,
    #[serde(default)]
    no_instance_id: bool,
    #[serde(default)]
    secure_time: bool,
}

impl Default for ManifestDefaults {
    fn default() -> Self {
        Self {
            manifest_version: 2,
            name: Some(VM_NAME.to_string()),
            runner: "docker-compose".to_string(),
            kms_enabled: true,
            gateway_enabled: true,
            local_key_provider_enabled: false,
            key_provider_id: String::new(),
            public_logs: true,
            public_sysinfo: true,
            public_tcbinfo: true,
            no_instance_id: false,
            secure_time: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VmParameters {
    #[serde(default)]
    name: Option<String>,
    image: String,
    vcpu: u32,
    memory: u32,
    disk_size: u32,
    #[serde(default)]
    user_config: String,
    #[serde(default)]
    ports: Vec<PortMapping>,
    #[serde(default)]
    hugepages: bool,
    #[serde(default)]
    pin_numa: bool,
    #[serde(default)]
    stopped: bool,
}

impl Default for VmParameters {
    fn default() -> Self {
        Self {
            name: Some(VM_NAME.to_string()),
            image: "dstack-0.5.2".to_string(),
            vcpu: 16,
            memory: 16 * 1024,
            disk_size: 200,
            user_config: String::new(),
            ports: Vec::new(),
            hugepages: false,
            pin_numa: false,
            stopped: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PortMapping {
    protocol: String,
    host_port: u16,
    vm_port: u16,
    #[serde(default)]
    host_address: Option<String>,
}

impl Default for PortMapping {
    fn default() -> Self {
        Self {
            protocol: "tcp".to_string(),
            host_port: 0,
            vm_port: 0,
            host_address: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformConfig {
    #[serde(default)]
    pub dstack_vmm_url: Option<String>,
    #[serde(default)]
    pub env: Option<std::collections::HashMap<String, String>>,
}

impl PlatformConfig {
    pub fn load() -> Result<Self> {
        let config_content = std::fs::read_to_string(PLATFORM_CONFIG_PATH)
            .context(format!("Failed to read {}", PLATFORM_CONFIG_PATH))?;

        let config =
            serde_json::from_str(&config_content).context("Failed to parse config JSON")?;

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(self).context("Failed to serialize config")?;

        std::fs::write(PLATFORM_CONFIG_PATH, json)
            .context(format!("Failed to write to {}", PLATFORM_CONFIG_PATH))?;

        Ok(())
    }

    pub fn ensure_env_map(&mut self) {
        if self.env.is_none() {
            self.env = Some(std::collections::HashMap::new());
        }
    }
}

#[derive(Parser)]
#[command(name = "validator-auto-updater")]
#[command(about = "Validator VM auto-updater and configuration manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the auto-updater service
    Run,
    /// Manage platform configuration
    Config {
        #[command(subcommand)]
        cmd: config_tui::ConfigCommands,
    },
}

struct ValidatorUpdater {
    vmm_url: String,
    http_client: reqwest::Client,
    current_hash: Option<String>,
    vm_id: Option<String>,
}

impl ValidatorUpdater {
    async fn new(vmm_url: String) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .danger_accept_invalid_certs(true)
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            vmm_url,
            http_client,
            current_hash: None,
            vm_id: None,
        })
    }

    async fn rpc_call(&self, method: &str, params: Value) -> Result<Value> {
        let url = format!("{}/prpc/{}?json", self.vmm_url, method);
        info!("Making RPC call to: {}", url);

        let response = self
            .http_client
            .post(&url)
            .json(&params)
            .send()
            .await
            .context("Failed to make RPC call")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            error!("RPC call failed with status {}: {}", status, error_text);
            anyhow::bail!("RPC call failed with status {}: {}", status, error_text);
        }

        response
            .json::<Value>()
            .await
            .context("Failed to parse RPC response")
    }

    async fn fetch_compose_config(&self) -> Result<ComposeConfig> {
        let response = self
            .http_client
            .get(API_URL)
            .send()
            .await
            .context("Failed to fetch compose config")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            error!("API returned status {}: {}", status, error_text);
            anyhow::bail!("API returned status {}: {}", status, error_text);
        }

        let response_text = response
            .text()
            .await
            .context("Failed to read response body")?;

        match serde_json::from_str::<ComposeConfig>(&response_text) {
            Ok(config) => Ok(config),
            Err(e) => {
                error!(
                    "Failed to parse compose config JSON. Response: {}",
                    response_text
                );
                Err(e).context("Failed to parse compose config")
            }
        }
    }

    fn compute_compose_hash(compose_content: &str, image_version: &str) -> String {
        // Normalize JSON to ensure consistent key ordering before hashing
        let normalized = Self::normalize_json_for_hashing(compose_content)
            .unwrap_or_else(|_| compose_content.to_string());

        // Include image version in hash to ensure VM is recreated when image changes
        let mut hasher = Sha256::new();
        hasher.update(normalized.as_bytes());
        hasher.update(b"\0"); // Separator
        hasher.update(image_version.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Normalize JSON by parsing and re-serializing with sorted keys
    /// This ensures consistent hashing regardless of key order
    fn normalize_json_for_hashing(json_str: &str) -> Result<String> {
        let value: serde_json::Value =
            serde_json::from_str(json_str).context("Failed to parse JSON for normalization")?;

        // Use to_string() which will serialize with consistent ordering
        // For objects, serde_json maintains insertion order, but we need to sort
        let normalized = Self::sort_json_keys(&value);

        serde_json::to_string(&normalized).context("Failed to serialize normalized JSON")
    }

    /// Recursively sort all object keys in a JSON value
    fn sort_json_keys(value: &serde_json::Value) -> serde_json::Value {
        use serde_json::Value;

        match value {
            Value::Object(map) => {
                let mut sorted: std::collections::BTreeMap<String, Value> =
                    std::collections::BTreeMap::new();
                for (k, v) in map {
                    sorted.insert(k.clone(), Self::sort_json_keys(v));
                }
                Value::Object(sorted.into_iter().collect())
            }
            Value::Array(arr) => {
                Value::Array(arr.iter().map(|v| Self::sort_json_keys(v)).collect())
            }
            _ => value.clone(),
        }
    }

    fn load_platform_config(&self) -> Result<PlatformConfig> {
        PlatformConfig::load()
    }

    fn build_env_vars(&self, platform_config: &PlatformConfig) -> Vec<Value> {
        let mut env_vars = Vec::new();
        let mut seen_keys = std::collections::HashSet::<String>::new();

        // First, add all environment variables from platform config.env
        // These are the values set via "config set-env" command for API-required keys
        if let Some(custom_env) = &platform_config.env {
            for (key, value) in custom_env {
                env_vars.push(json!({
                    "key": key,
                    "value": value
                }));
                seen_keys.insert(key.clone());
            }
        }

        // Add DSTACK_VMM_URL (always added from platform config, unless already in env)
        if !seen_keys.contains("DSTACK_VMM_URL") {
            let vmm_url = platform_config
                .dstack_vmm_url
                .clone()
                .unwrap_or_else(|| "http://10.0.2.2:10300/".to_string());

            env_vars.push(json!({
                "key": "DSTACK_VMM_URL",
                "value": vmm_url
            }));
            seen_keys.insert("DSTACK_VMM_URL".to_string());
        }

        info!(
            "Built {} environment variables for VM from platform config",
            env_vars.len()
        );
        env_vars
    }

    fn validate_vm_parameters(params: &VmParameters) -> Result<()> {
        if params.vcpu == 0 {
            anyhow::bail!("Validator VM configuration must specify at least one vCPU");
        }
        if params.memory == 0 {
            anyhow::bail!("Validator VM configuration must specify memory in MB (> 0)");
        }
        if params.disk_size == 0 {
            anyhow::bail!("Validator VM configuration must specify disk_size in GB (> 0)");
        }
        Ok(())
    }

    fn log_vm_parameters(vm_type: &str, params: &VmParameters) {
        info!(
            target: "validator-updater",
            "Validator VM hardware spec resolved: vm_type={}, image={}, vcpu={}, memory_mb={}, disk_gb={}",
            vm_type,
            params.image,
            params.vcpu,
            params.memory,
            params.disk_size,
        );
    }

    fn check_required_env(
        &self,
        required_env: &[String],
        env_vars: &[Value],
    ) -> Result<Vec<String>> {
        let mut missing = Vec::new();

        for required in required_env {
            let found = env_vars.iter().any(|env| {
                env.get("key")
                    .and_then(|k| k.as_str())
                    .map(|k| k == required)
                    .unwrap_or(false)
            });

            if !found {
                missing.push(required.clone());
            }
        }

        Ok(missing)
    }

    async fn ensure_required_env(&self, required_env_keys: &[String]) -> Result<()> {
        if required_env_keys.is_empty() {
            return Ok(());
        }

        let platform_config = self
            .load_platform_config()
            .unwrap_or_else(|_| PlatformConfig {
                dstack_vmm_url: Some("http://10.0.2.2:10300/".to_string()),
                env: None,
            });

        // Build env vars from platform config (merges API keys with local values)
        let env_vars = self.build_env_vars(&platform_config);

        // Check which required keys are missing values
        let missing = self.check_required_env(required_env_keys, &env_vars)?;

        if !missing.is_empty() {
            error!(
                "Missing values for required environment variable keys: {:?}",
                missing
            );
            anyhow::bail!(
                "Missing values for required environment variable keys: {}. Please set them with 'validator-auto-updater config set-env <key> <value>'",
                missing.join(", ")
            );
        }

        Ok(())
    }

    async fn find_validator_vm(&mut self) -> Result<Option<(String, String, Option<String>)>> {
        let response = self
            .rpc_call("Status", json!({}))
            .await
            .context("Failed to get VM status")?;

        let vms = response
            .get("vms")
            .and_then(|v| v.as_array())
            .context("Invalid status response")?;

        for vm in vms {
            let name = vm.get("name").and_then(|n| n.as_str());
            let app_id = vm.get("appId").and_then(|a| a.as_str()).or_else(|| {
                // Try alternative field name
                vm.get("app_id").and_then(|a| a.as_str())
            });
            let id = vm.get("id").and_then(|i| i.as_str());
            let status = vm
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("unknown");

            if (name == Some(VM_NAME) || app_id == Some(VM_NAME)) && id.is_some() {
                if app_id.is_none() {
                    warn!(
                        "Found VM {} but appId is missing. VM data: {}",
                        id.unwrap(),
                        serde_json::to_string(vm)
                            .unwrap_or_else(|_| "failed to serialize".to_string())
                    );
                }

                return Ok(Some((
                    id.unwrap().to_string(),
                    status.to_string(),
                    app_id.map(String::from),
                )));
            }
        }
        Ok(None)
    }

    async fn stop_vm(&self, vm_id: &str) -> Result<()> {
        info!("Stopping VM: {}", vm_id);

        match timeout(
            VM_KILL_TIMEOUT,
            self.rpc_call("StopVm", json!({ "id": vm_id })),
        )
        .await
        {
            Ok(Ok(_)) => {
                info!("VM {} stop command sent, waiting for VM to stop...", vm_id);
                // Wait for VM to actually stop
                sleep(Duration::from_secs(5)).await;
                Ok(())
            }
            Ok(Err(e)) => {
                warn!(
                    "Failed to stop VM {}: {}, will try to remove anyway",
                    vm_id, e
                );
                Ok(()) // Don't fail, just warn
            }
            Err(_) => {
                warn!("Timeout stopping VM {}", vm_id);
                Ok(()) // Don't fail, just warn
            }
        }
    }

    async fn remove_vm(&self, vm_id: &str) -> Result<()> {
        info!("Removing VM: {}", vm_id);

        // Retry removal up to 3 times with delays
        for attempt in 1..=3 {
            match self.rpc_call("RemoveVm", json!({ "id": vm_id })).await {
                Ok(_) => {
                    info!("VM {} removed successfully", vm_id);
                    return Ok(());
                }
                Err(e) => {
                    if attempt < 3 {
                        warn!(
                            "Failed to remove VM {} (attempt {}/3): {}, retrying...",
                            vm_id, attempt, e
                        );
                        sleep(Duration::from_secs(3)).await;
                    } else {
                        error!("Failed to remove VM {} after 3 attempts", vm_id);
                        return Err(e);
                    }
                }
            }
        }

        Ok(())
    }

    async fn kill_and_remove_vm(&self, vm_id: &str) -> Result<()> {
        info!("Killing and removing VM: {}", vm_id);

        // Always stop first (won't fail even if error)
        let _ = self.stop_vm(vm_id).await;

        // Wait a bit more to ensure VM is fully stopped
        sleep(Duration::from_secs(2)).await;

        // Try to remove with retries
        self.remove_vm(vm_id).await?;

        info!("VM {} successfully killed and removed", vm_id);
        Ok(())
    }

    async fn create_vm(
        &self,
        compose_config: &ComposeConfig,
        compose_hash: &str,
        vm_params: &VmParameters,
    ) -> Result<String> {
        info!(
            "Creating new VM with compose hash: {} (image: {})",
            compose_hash, vm_params.image
        );

        // Load platform configuration (always use values from platform config)
        let platform_config = self.load_platform_config().unwrap_or_else(|e| {
            warn!("Failed to load platform config: {}, using defaults", e);
            PlatformConfig {
                dstack_vmm_url: Some("http://10.0.2.2:10300/".to_string()),
                env: None,
            }
        });

        info!(
            "Loaded platform config for VM creation: VMM URL={:?}, env vars count={}",
            platform_config.dstack_vmm_url,
            platform_config.env.as_ref().map(|e| e.len()).unwrap_or(0)
        );

        // Build environment variables from platform config
        let env_vars = self.build_env_vars(&platform_config);

        // Build allowed_envs list from API config to ensure hash consistency
        // We must ONLY use keys that platform-api expects (provisioning.env_keys)
        // Extra local env vars must NOT be in allowed_envs or the compose hash will mismatch
        let mut allowed_envs = compose_config.provisioning.env_keys.clone();

        // Add required env keys that platform-api expects (DEFAULT_ENV_KEYS)
        for key in &["DSTACK_VMM_URL", "HOTKEY_PASSPHRASE", "VALIDATOR_BASE_URL"] {
            if !allowed_envs.contains(&key.to_string()) {
                info!("Adding missing required env key: {}", key);
                allowed_envs.push(key.to_string());
            }
        }

        // Add compose config required_env keys
        for key in &compose_config.required_env {
            if !allowed_envs.contains(key) {
                info!(
                    "Adding missing required_env key from compose config: {}",
                    key
                );
                allowed_envs.push(key.clone());
            }
        }

        // Remove duplicates and sort for stable hash computation
        allowed_envs.sort();
        allowed_envs.dedup();

        info!("Allowed environment variables: {:?}", allowed_envs);
        info!(
            "Number of allowed environment variables: {}",
            allowed_envs.len()
        );

        // Create app_compose structure
        let vm_name = vm_params
            .name
            .clone()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| compose_config.vm_type.clone());

        let app_compose = Self::build_app_manifest(
            &compose_config.compose_content,
            &compose_config.provisioning.manifest_defaults,
            &vm_name,
            &allowed_envs,
        );

        // Serialize app_compose to JSON string for compose_file
        let compose_file_str =
            serde_json::to_string(&app_compose).context("Failed to serialize app_compose")?;

        // Calculate app_id for encryption (includes image version)
        let app_id = Self::compute_compose_hash(&compose_file_str, &vm_params.image);
        let app_id_truncated = &app_id[..40];

        info!("Computed compose hash (app_id): {}", app_id);

        // Get encryption public key from KMS
        info!("Getting encryption key for app_id: {}", app_id_truncated);
        let pubkey_response = self
            .rpc_call(
                "GetAppEnvEncryptPubKey",
                json!({
                    "app_id": app_id_truncated
                }),
            )
            .await
            .context("Failed to get encryption public key")?;

        let pubkey_hex = pubkey_response
            .get("public_key")
            .and_then(|k| k.as_str())
            .context("Invalid public key response")?;

        // Encrypt environment variables
        let env_to_encrypt = json!(env_vars);

        let encrypted_env = self.encrypt_env(&env_to_encrypt.to_string(), pubkey_hex)?;

        Self::validate_vm_parameters(vm_params)?;

        let vm_config = json!({
            "name": vm_params.name.clone().unwrap_or_else(|| vm_name.clone()),
            "image": vm_params.image,
            "compose_file": compose_file_str,
            "vcpu": vm_params.vcpu,
            "memory": vm_params.memory,
            "disk_size": vm_params.disk_size,
            "user_config": vm_params.user_config,
            "ports": vm_params.ports,
            "encrypted_env": encrypted_env,
            "hugepages": vm_params.hugepages,
            "pin_numa": vm_params.pin_numa,
            "stopped": vm_params.stopped,
        });

        // Get the compose hash from VMM to validate
        let hash_response = self
            .rpc_call("GetComposeHash", vm_config.clone())
            .await
            .context("Failed to get compose hash from VMM")?;

        let vmm_hash = hash_response
            .get("hash")
            .and_then(|h| h.as_str())
            .context("Invalid hash response")?;

        info!("VMM computed compose hash: {}", vmm_hash);

        // Create the VM
        let response = self
            .rpc_call("CreateVm", vm_config)
            .await
            .context("Failed to create VM")?;

        let vm_id = response
            .get("id")
            .and_then(|i| i.as_str())
            .context("Invalid create VM response")?
            .to_string();

        info!("VM created with ID: {}", vm_id);
        Ok(vm_id)
    }

    fn build_app_manifest(
        compose_content: &str,
        defaults: &ManifestDefaults,
        vm_name: &str,
        allowed_envs: &[String],
    ) -> Value {
        json!({
            "manifest_version": defaults.manifest_version,
            "name": defaults
                .name
                .clone()
                .unwrap_or_else(|| vm_name.to_string()),
            "runner": defaults.runner.clone(),
            "docker_compose_file": compose_content,
            "kms_enabled": defaults.kms_enabled,
            "gateway_enabled": defaults.gateway_enabled,
            "local_key_provider_enabled": defaults.local_key_provider_enabled,
            "key_provider_id": defaults.key_provider_id.clone(),
            "public_logs": defaults.public_logs,
            "public_sysinfo": defaults.public_sysinfo,
            "public_tcbinfo": defaults.public_tcbinfo,
            "allowed_envs": allowed_envs,
            "no_instance_id": defaults.no_instance_id,
            "secure_time": defaults.secure_time,
        })
    }

    fn encrypt_env(&self, env_json: &str, pubkey_hex: &str) -> Result<String> {
        // Serialize environment variables to JSON with "env" wrapper
        let env_data = format!(r#"{{"env":{}}}"#, env_json);
        let env_bytes = env_data.as_bytes();

        // Remove "0x" prefix if present
        let pubkey_hex = pubkey_hex.strip_prefix("0x").unwrap_or(pubkey_hex);

        // Decode the remote public key
        let remote_pubkey_bytes =
            hex::decode(pubkey_hex).context("Failed to decode public key hex")?;

        if remote_pubkey_bytes.len() != 32 {
            anyhow::bail!(
                "Invalid public key length: expected 32 bytes, got {}",
                remote_pubkey_bytes.len()
            );
        }

        let remote_pubkey_array: [u8; 32] = remote_pubkey_bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("Failed to convert public key to array"))?;
        let remote_pubkey = PublicKey::from(remote_pubkey_array);

        // Generate ephemeral X25519 key pair
        let ephemeral_secret = EphemeralSecret::random_from_rng(rand::thread_rng());
        let ephemeral_public = PublicKey::from(&ephemeral_secret);

        // Compute shared secret using X25519 key exchange
        let shared_secret = ephemeral_secret.diffie_hellman(&remote_pubkey);

        // Use shared secret as AES-256-GCM key (32 bytes)
        let cipher = Aes256Gcm::new(shared_secret.as_bytes().into());

        // Generate random 12-byte nonce (IV) for AES-GCM
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt the environment data
        let ciphertext = cipher
            .encrypt(nonce, env_bytes)
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

        // Combine: ephemeral_public_key (32 bytes) + nonce (12 bytes) + ciphertext
        let mut result = Vec::new();
        result.extend_from_slice(ephemeral_public.as_bytes());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        // Return as hex string
        Ok(hex::encode(result))
    }

    async fn check_and_update(&mut self) -> Result<()> {
        // Fetch latest compose config
        let config = self.fetch_compose_config().await?;

        // Collect required environment variable keys from API
        // These are just keys - values come from platform config
        let mut required_env_keys = config.required_env.clone();
        for key in &config.provisioning.env_keys {
            if !required_env_keys.iter().any(|existing| existing == key) {
                required_env_keys.push(key.clone());
            }
        }
        if !required_env_keys.is_empty() {
            info!(
                "Required environment variable keys from API: {:?}",
                required_env_keys
            );
            self.ensure_required_env(&required_env_keys).await?;
        }

        // Load platform configuration (must be loaded to use values from platform config)
        let platform_config = self.load_platform_config().unwrap_or_else(|e| {
            warn!("Failed to load platform config: {}, using defaults", e);
            PlatformConfig {
                dstack_vmm_url: Some("http://10.0.2.2:10300/".to_string()),
                env: None,
            }
        });

        info!(
            "Loaded platform config: VMM URL={:?}, env vars count={}",
            platform_config.dstack_vmm_url,
            platform_config.env.as_ref().map(|e| e.len()).unwrap_or(0)
        );

        // Build allowed_envs list from API config to ensure hash consistency
        // We must ONLY use keys that platform-api expects (provisioning.env_keys)
        // Extra local env vars must NOT be in allowed_envs or the compose hash will mismatch
        let mut allowed_envs = config.provisioning.env_keys.clone();

        // Add required env keys that platform-api expects (DEFAULT_ENV_KEYS)
        for key in &["DSTACK_VMM_URL", "HOTKEY_PASSPHRASE", "VALIDATOR_BASE_URL"] {
            if !allowed_envs.contains(&key.to_string()) {
                info!("Adding missing required env key: {}", key);
                allowed_envs.push(key.to_string());
            }
        }

        // Add compose config required_env keys
        for key in &config.required_env {
            if !allowed_envs.contains(key) {
                info!(
                    "Adding missing required_env key from compose config: {}",
                    key
                );
                allowed_envs.push(key.clone());
            }
        }

        // Remove duplicates and sort for stable hash computation
        allowed_envs.sort();
        allowed_envs.dedup();

        info!("Allowed environment variables: {:?}", allowed_envs);
        info!(
            "Number of allowed environment variables: {}",
            allowed_envs.len()
        );

        let vm_params = config.provisioning.vm_parameters.clone();

        Self::validate_vm_parameters(&vm_params)?;
        Self::log_vm_parameters(&config.vm_type, &vm_params);

        // Use VM name from API config (or fallback to vm_type)
        let vm_name = vm_params
            .name
            .clone()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| config.vm_type.clone());

        // Build app manifest using values from API config (manifest_defaults come from API)
        // but environment variables come from platform config
        let app_compose = Self::build_app_manifest(
            &config.compose_content,
            &config.provisioning.manifest_defaults,
            &vm_name,
            &allowed_envs,
        );

        let compose_file_str =
            serde_json::to_string(&app_compose).context("Failed to serialize app_compose")?;

        // Calculate hash the same way as in create_vm (on the JSON stringified app_compose)
        // Include image version in hash to ensure VM is recreated when image changes
        let new_hash = Self::compute_compose_hash(&compose_file_str, &vm_params.image);

        info!(
            "Computed compose hash (image: {}): {}",
            vm_params.image, new_hash
        );

        // Find existing VM and its status
        let vm_info = self.find_validator_vm().await?;

        let is_first_run = self.current_hash.is_none();

        // Check if VM exists and verify its compose hash
        let should_recreate = if let Some((vm_id, status, vm_app_id)) = &vm_info {
            // Check if VM is stopped, exited, or killed
            let is_stopped = matches!(status.as_str(), "stopped" | "exited" | "killed" | "error");

            if is_stopped {
                warn!("VM is in '{}' state, will recreate", status);
                true
            } else if let Some(existing_app_id) = vm_app_id {
                // VM is running and we have its app_id, check if compose hash matches
                // Compare with the first 40 chars (as app_id is truncated to 40 chars)
                let new_hash_truncated = &new_hash[..40.min(new_hash.len())];
                let existing_hash_truncated = &existing_app_id[..40.min(existing_app_id.len())];

                info!(
                    "Comparing compose hashes - existing VM: {}, new config: {}",
                    existing_hash_truncated, new_hash_truncated
                );

                if existing_hash_truncated == new_hash_truncated {
                    if is_first_run {
                        info!("Existing VM found at startup with status '{}' and matching compose hash ({}), keeping it", status, existing_hash_truncated);
                        self.vm_id = Some(vm_id.clone());
                        self.current_hash = Some(new_hash);
                        return Ok(());
                    } else {
                        info!(
                            "VM compose hash matches ({}), no update needed",
                            existing_hash_truncated
                        );
                        self.vm_id = Some(vm_id.clone());
                        self.current_hash = Some(new_hash);
                        return Ok(());
                    }
                } else {
                    info!(
                        "VM compose hash mismatch: existing={}, new={}, will recreate",
                        existing_hash_truncated, new_hash_truncated
                    );
                    true
                }
            } else {
                warn!("VM exists but has no appId (compose hash), will recreate to ensure consistency");
                true
            }
        } else {
            // No VM exists, need to create
            info!("No existing VM found, will create new one");
            true
        };

        // Kill and remove existing VM if it exists and needs recreation
        if should_recreate {
            if let Some((vm_id, _, _)) = vm_info {
                info!("Killing and removing existing VM: {}", vm_id);
                if let Err(e) = self.kill_and_remove_vm(&vm_id).await {
                    error!("Failed to kill/remove VM: {}", e);
                    return Err(e);
                }
                self.vm_id = None;
            }
        } else {
            // VM is fine, no action needed
            return Ok(());
        }

        // Create new VM with updated compose
        let new_vm_id = self.create_vm(&config, &new_hash, &vm_params).await?;

        // Update state
        self.vm_id = Some(new_vm_id.clone());
        self.current_hash = Some(new_hash);

        info!("VM updated successfully!");
        Ok(())
    }

    async fn run(&mut self) -> Result<()> {
        info!("Starting validator auto-updater");
        info!("Polling {} every {:?}", API_URL, POLL_INTERVAL);

        // Initial check
        if let Err(e) = self.check_and_update().await {
            error!("Initial check failed: {}", e);
        }

        // Poll loop
        loop {
            sleep(POLL_INTERVAL).await;

            if let Err(e) = self.check_and_update().await {
                error!("Update check failed: {}", e);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Config { cmd } => {
            return config_tui::run_config_command(cmd);
        }
        Commands::Run => {
            // Continue to run the auto-updater
        }
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let vmm_url = std::env::var("VMM_URL").unwrap_or_else(|_| "http://localhost:10300".to_string());

    info!("Connecting to VMM at: {}", vmm_url);

    let mut updater = ValidatorUpdater::new(vmm_url)
        .await
        .context("Failed to initialize updater")?;

    updater.run().await
}
