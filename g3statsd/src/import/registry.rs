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

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use foldhash::fast::FixedState;

use g3_types::metrics::NodeName;

use super::ArcImporter;
use crate::config::importer::AnyImporterConfig;

static RUNTIME_IMPORTER_REGISTRY: Mutex<ImporterRegistry> = Mutex::new(ImporterRegistry::new());

pub(crate) struct ImporterRegistry {
    inner: HashMap<NodeName, ArcImporter, FixedState>,
}

impl ImporterRegistry {
    const fn new() -> Self {
        ImporterRegistry {
            inner: HashMap::with_hasher(FixedState::with_seed(0)),
        }
    }

    fn add(&mut self, name: NodeName, importer: ArcImporter) -> anyhow::Result<()> {
        importer._start_runtime(&importer)?;
        if let Some(old_importer) = self.inner.insert(name, importer) {
            old_importer._abort_runtime();
        }
        Ok(())
    }

    fn del(&mut self, name: &NodeName) {
        if let Some(old_importer) = self.inner.remove(name) {
            old_importer._abort_runtime();
        }
    }

    fn get_names(&self) -> HashSet<NodeName> {
        self.inner.keys().cloned().collect()
    }

    fn get_config(&self, name: &NodeName) -> Option<AnyImporterConfig> {
        self.inner
            .get(name)
            .map(|importer| importer._clone_config())
    }

    fn get_importer(&self, name: &NodeName) -> Option<ArcImporter> {
        self.inner.get(name).cloned()
    }

    fn reload_no_respawn(
        &mut self,
        name: &NodeName,
        config: AnyImporterConfig,
    ) -> anyhow::Result<()> {
        let Some(old_importer) = self.inner.get(name) else {
            return Err(anyhow!("no importer with name {name} found"));
        };

        let old_importer = old_importer.clone();
        let importer = old_importer._reload_with_old_notifier(config, self)?;
        if let Some(_old_importer) = self.inner.insert(name.clone(), Arc::clone(&importer)) {
            // do not abort the runtime, as it's reused
        }
        importer._reload_config_notify_runtime();
        Ok(())
    }

    fn reload_and_respawn(
        &mut self,
        name: &NodeName,
        config: AnyImporterConfig,
    ) -> anyhow::Result<()> {
        let Some(old_importer) = self.inner.get(name) else {
            return Err(anyhow!("no importer with name {name} found"));
        };

        let old_importer = old_importer.clone();
        let importer = old_importer._reload_with_new_notifier(config, self)?;
        importer._start_runtime(&importer)?;
        if let Some(old_importer) = self.inner.insert(name.clone(), importer) {
            old_importer._abort_runtime();
        }
        Ok(())
    }

    fn foreach<F>(&self, mut f: F)
    where
        F: FnMut(&NodeName, &ArcImporter),
    {
        for (name, importer) in self.inner.iter() {
            f(name, importer)
        }
    }

    pub(crate) fn get_or_insert_default(&mut self, name: &NodeName) -> ArcImporter {
        self.inner
            .entry(name.clone())
            .or_insert_with(|| super::dummy::DummyImporter::prepare_default(name))
            .clone()
    }
}

pub(super) fn add(name: NodeName, importer: ArcImporter) -> anyhow::Result<()> {
    let mut sr = RUNTIME_IMPORTER_REGISTRY
        .lock()
        .map_err(|e| anyhow!("failed to lock importer registry: {e}"))?;
    sr.add(name, importer)
}

pub(super) fn del(name: &NodeName) {
    let mut sr = RUNTIME_IMPORTER_REGISTRY.lock().unwrap();
    sr.del(name);
}

pub(crate) fn get_names() -> HashSet<NodeName> {
    let sr = RUNTIME_IMPORTER_REGISTRY.lock().unwrap();
    sr.get_names()
}

pub(super) fn get_config(name: &NodeName) -> Option<AnyImporterConfig> {
    let sr = RUNTIME_IMPORTER_REGISTRY.lock().unwrap();
    sr.get_config(name)
}

pub(super) fn reload_no_respawn(name: &NodeName, config: AnyImporterConfig) -> anyhow::Result<()> {
    let mut sr = RUNTIME_IMPORTER_REGISTRY
        .lock()
        .map_err(|e| anyhow!("failed to lock importer registry: {e}"))?;
    sr.reload_no_respawn(name, config)
}

pub(crate) fn get_importer(name: &NodeName) -> Option<ArcImporter> {
    let sr = RUNTIME_IMPORTER_REGISTRY.lock().unwrap();
    sr.get_importer(name)
}

pub(crate) fn reload_only_collector(name: &NodeName) -> anyhow::Result<()> {
    let Some(importer) = get_importer(name) else {
        return Err(anyhow!("no importer with name {name} found"));
    };
    importer._update_collector_in_place();
    Ok(())
}

pub(super) fn reload_and_respawn(name: &NodeName, config: AnyImporterConfig) -> anyhow::Result<()> {
    let mut sr = RUNTIME_IMPORTER_REGISTRY
        .lock()
        .map_err(|e| anyhow!("failed to lock importer registry: {e}"))?;
    sr.reload_and_respawn(name, config)
}

pub(crate) fn foreach<F>(f: F)
where
    F: FnMut(&NodeName, &ArcImporter),
{
    let sr = RUNTIME_IMPORTER_REGISTRY.lock().unwrap();
    sr.foreach(f)
}

pub(crate) fn get_or_insert_default(name: &NodeName) -> ArcImporter {
    let mut sr = RUNTIME_IMPORTER_REGISTRY.lock().unwrap();
    sr.get_or_insert_default(name)
}
