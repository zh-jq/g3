/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::sync::Arc;
use std::time::Duration;

use log::{info, warn};
use serde_json::{Map, Value};

use g3_daemon::register::{RegisterConfig, RegisterTask};

pub async fn startup(config: Arc<RegisterConfig>, ctl_socket_path: String) -> anyhow::Result<()> {
    let mut data = serde_json::Map::new();
    data.insert("ctl_local".to_string(), Value::String(ctl_socket_path));
    data.insert(
        "pid".to_string(),
        Value::String(std::process::id().to_string()),
    );

    let mut retry_count = 0;
    loop {
        match register(config.clone(), data.clone()).await {
            Ok(mut task) => {
                info!("process register ok");

                tokio::spawn(async move {
                    loop {
                        if let Err(e) = task.ping_until_end().await {
                            warn!("lost connection with register upstream: {e:?}");
                        }

                        loop {
                            tokio::time::sleep(Duration::from_secs(10)).await;
                            info!("start reconnect to register upstream");
                            if let Err(e) = task.reopen().await {
                                warn!("reconnect to register upstream failed: {e:?}");
                                continue;
                            }
                            info!("reconnected to register upstream");
                            if let Err(e) = task.register(data.clone()).await {
                                warn!("register failed: {e:?}");
                                continue;
                            }
                            info!("process register ok");
                            break;
                        }
                    }
                });

                return Ok(());
            }
            Err(e) => {
                if retry_count < config.startup_retry() {
                    warn!("{retry_count} process register failed: {e:?}");
                    retry_count += 1;
                    tokio::time::sleep(config.retry_interval()).await;
                } else {
                    return Err(e);
                }
            }
        }
    }
}

async fn register(
    config: Arc<RegisterConfig>,
    data: Map<String, Value>,
) -> anyhow::Result<RegisterTask> {
    let mut task = RegisterTask::new(config).await?;
    task.register(data.clone()).await?;
    Ok(task)
}
