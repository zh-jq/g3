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

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_daemon::config::TopoMap;
use g3_types::metrics::NodeName;
use g3_yaml::{HybridParser, YamlDocPosition};

mod registry;
pub(crate) use registry::clear;

pub(crate) mod dummy;
pub(crate) mod internal;

const CONFIG_KEY_COLLECT_TYPE: &str = "type";
const CONFIG_KEY_COLLECT_NAME: &str = "name";

pub(crate) enum CollectConfigDiffAction {
    NoAction,
    SpawnNew,
    ReloadOnlyConfig,
    ReloadAndRespawn,
}

pub(crate) trait CollectConfig {
    fn name(&self) -> &NodeName;
    fn position(&self) -> Option<YamlDocPosition>;
    fn collect_type(&self) -> &'static str;

    fn diff_action(&self, new: &AnyCollectConfig) -> CollectConfigDiffAction;

    fn dependent_collecter(&self) -> Option<BTreeSet<NodeName>> {
        None
    }
}

#[derive(Clone, Debug)]
pub(crate) enum AnyCollectConfig {
    Dummy(dummy::DummyCollectConfig),
    Internal(internal::InternalCollectConfig),
}

macro_rules! impl_transparent0 {
    ($f:tt, $v:ty) => {
        pub(crate) fn $f(&self) -> $v {
            match self {
                AnyCollectConfig::Dummy(s) => s.$f(),
                AnyCollectConfig::Internal(s) => s.$f(),
            }
        }
    };
}

macro_rules! impl_transparent1 {
    ($f:tt, $v:ty, $p:ty) => {
        pub(crate) fn $f(&self, p: $p) -> $v {
            match self {
                AnyCollectConfig::Dummy(s) => s.$f(p),
                AnyCollectConfig::Internal(s) => s.$f(p),
            }
        }
    };
}

impl AnyCollectConfig {
    impl_transparent0!(name, &NodeName);
    impl_transparent0!(position, Option<YamlDocPosition>);
    impl_transparent0!(collect_type, &'static str);
    impl_transparent0!(dependent_collecter, Option<BTreeSet<NodeName>>);

    impl_transparent1!(diff_action, CollectConfigDiffAction, &Self);
}

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, g3_daemon::opts::config_file_extension());
    parser.foreach_map(v, |map, position| {
        let collect = load_collect(map, position)?;
        if let Some(old_collect) = registry::add(collect) {
            Err(anyhow!(
                "collect with name {} already exists",
                old_collect.name()
            ))
        } else {
            Ok(())
        }
    })?;
    build_topology_map()?;
    Ok(())
}

pub(crate) fn load_at_position(position: &YamlDocPosition) -> anyhow::Result<AnyCollectConfig> {
    let doc = g3_yaml::load_doc(position)?;
    if let Yaml::Hash(map) = doc {
        let collect = load_collect(&map, Some(position.clone()))?;
        let old_collect = registry::add(collect.clone());
        if let Err(e) = build_topology_map() {
            // rollback
            match old_collect {
                Some(collect) => {
                    registry::add(collect);
                }
                None => registry::del(collect.name()),
            }
            Err(e)
        } else {
            Ok(collect)
        }
    } else {
        Err(anyhow!("yaml doc {position} is not a map"))
    }
}

fn load_collect(
    map: &yaml::Hash,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<AnyCollectConfig> {
    let collect_type = g3_yaml::hash_get_required_str(map, CONFIG_KEY_COLLECT_TYPE)?;
    match g3_yaml::key::normalize(collect_type).as_str() {
        "dummy" => {
            let collect = dummy::DummyCollectConfig::parse(map, position)
                .context("failed to load this Dummy collect")?;
            Ok(AnyCollectConfig::Dummy(collect))
        }
        "internal" => {
            let collect = internal::InternalCollectConfig::parse(map, position)
                .context("failed to load this Internal collect")?;
            Ok(AnyCollectConfig::Internal(collect))
        }
        _ => Err(anyhow!("unsupported collect type {}", collect_type)),
    }
}

fn build_topology_map() -> anyhow::Result<TopoMap> {
    let mut topo_map = TopoMap::default();

    for name in registry::get_all_names() {
        topo_map.add_node(&name, &|name| {
            let conf = registry::get(name)?;
            conf.dependent_collecter()
        })?;
    }

    Ok(topo_map)
}

pub(crate) fn get_all_sorted() -> anyhow::Result<Vec<Arc<AnyCollectConfig>>> {
    let topo_map = build_topology_map()?;
    let sorted_nodes = topo_map.sorted_nodes();
    let mut sorted_conf = Vec::with_capacity(sorted_nodes.len());
    for node in sorted_nodes {
        let Some(conf) = registry::get(node.name()) else {
            continue;
        };
        sorted_conf.push(conf);
    }
    Ok(sorted_conf)
}
