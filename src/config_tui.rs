// SPDX-FileCopyrightText: © 2024-2025 Phala Network <dstack@phala.network>
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Subcommand;

use crate::PlatformConfig;

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Show current configuration
    Show,
    /// Set VMM URL
    SetVmmUrl {
        /// VMM URL (e.g., http://10.0.2.2:16850/)
        url: String,
    },
    /// Set an environment variable
    SetEnv {
        /// Environment variable key
        key: String,
        /// Environment variable value
        value: String,
    },
    /// Remove an environment variable
    RemoveEnv {
        /// Environment variable key to remove
        key: String,
    },
    /// List all environment variables
    ListEnv,
    /// Get a specific environment variable value
    GetEnv {
        /// Environment variable key
        key: String,
    },
}

pub fn run_config_command(cmd: ConfigCommands) -> Result<()> {
    let mut config = PlatformConfig::load().unwrap_or_else(|_| PlatformConfig {
        dstack_vmm_url: Some("http://10.0.2.2:16850/".to_string()),
        env: None,
    });

    match cmd {
        ConfigCommands::Show => {
            println!("Current Platform Configuration:");
            println!(
                "  VMM URL: {}",
                config.dstack_vmm_url.as_deref().unwrap_or("(not set)")
            );
            println!("  Environment Variables:");
            if let Some(env) = &config.env {
                if env.is_empty() {
                    println!("    (none)");
                } else {
                    for (key, value) in env {
                        println!("    {} = {}", key, value);
                    }
                }
            } else {
                println!("    (none)");
            }
        }
        ConfigCommands::SetVmmUrl { url } => {
            config.dstack_vmm_url = Some(url.clone());
            config.save()?;
            println!("✓ VMM URL set to: {}", url);
        }
        ConfigCommands::SetEnv { key, value } => {
            config.ensure_env_map();
            config
                .env
                .as_mut()
                .unwrap()
                .insert(key.clone(), value.clone());
            config.save()?;
            println!("✓ Environment variable set: {} = {}", key, value);
        }
        ConfigCommands::RemoveEnv { key } => {
            if let Some(env) = &mut config.env {
                if env.remove(&key).is_some() {
                    config.save()?;
                    println!("✓ Environment variable removed: {}", key);
                } else {
                    anyhow::bail!("Environment variable '{}' not found", key);
                }
            } else {
                anyhow::bail!("No environment variables configured");
            }
        }
        ConfigCommands::ListEnv => {
            if let Some(env) = &config.env {
                if env.is_empty() {
                    println!("No environment variables configured");
                } else {
                    println!("Environment Variables:");
                    for (key, value) in env {
                        println!("  {} = {}", key, value);
                    }
                }
            } else {
                println!("No environment variables configured");
            }
        }
        ConfigCommands::GetEnv { key } => {
            if let Some(env) = &config.env {
                if let Some(value) = env.get(&key) {
                    println!("{}", value);
                } else {
                    anyhow::bail!("Environment variable '{}' not found", key);
                }
            } else {
                anyhow::bail!("Environment variable '{}' not found", key);
            }
        }
    }

    Ok(())
}
