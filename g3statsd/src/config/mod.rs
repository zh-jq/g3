/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use std::path::Path;

use anyhow::anyhow;
use yaml_rust::{Yaml, yaml};

pub(crate) mod collector;
pub(crate) mod exporter;
pub(crate) mod importer;

pub fn load() -> anyhow::Result<&'static Path> {
    let config_file =
        g3_daemon::opts::config_file().ok_or_else(|| anyhow!("no config file set"))?;

    // allow multiple docs, and treat them as the same
    g3_yaml::foreach_doc(config_file, |_, doc| match doc {
        Yaml::Hash(map) => load_doc(map),
        _ => Err(anyhow!("yaml doc root should be hash")),
    })?;

    Ok(config_file)
}

pub(crate) async fn reload() -> anyhow::Result<()> {
    tokio::task::spawn_blocking(reload_blocking)
        .await
        .map_err(|e| anyhow!("failed to join reload task: {e}"))?
}

fn clear_all() {
    importer::clear();
    collector::clear();
    exporter::clear();
}

fn reload_blocking() -> anyhow::Result<()> {
    clear_all();
    if let Some(conf_file) = g3_daemon::opts::config_file() {
        // allow multiple docs, and treat them as the same
        g3_yaml::foreach_doc(conf_file, |_, doc| match doc {
            Yaml::Hash(map) => reload_doc(map),
            _ => Err(anyhow!("yaml doc root should be hash")),
        })?;
    }
    Ok(())
}

fn reload_doc(map: &yaml::Hash) -> anyhow::Result<()> {
    let conf_dir =
        g3_daemon::opts::config_dir().ok_or_else(|| anyhow!("no valid config dir has been set"))?;
    g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
        "runtime" | "worker" | "log" | "controller" => Ok(()),
        "importer" => importer::load_all(v, conf_dir),
        "collector" => collector::load_all(v, conf_dir),
        "exporter" => exporter::load_all(v, conf_dir),
        _ => Ok(()),
    })?;
    Ok(())
}

fn load_doc(map: &yaml::Hash) -> anyhow::Result<()> {
    let conf_dir =
        g3_daemon::opts::config_dir().ok_or_else(|| anyhow!("no valid config dir has been set"))?;
    g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
        "runtime" => g3_daemon::runtime::config::load(v),
        "worker" => g3_daemon::runtime::config::load_worker(v),
        "importer" => importer::load_all(v, conf_dir),
        "collector" => collector::load_all(v, conf_dir),
        "exporter" => exporter::load_all(v, conf_dir),
        _ => Err(anyhow!("invalid key {k} in main conf")),
    })?;
    Ok(())
}
