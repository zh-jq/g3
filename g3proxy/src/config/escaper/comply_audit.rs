/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use anyhow::anyhow;
use yaml_rust::{yaml, Yaml};

use g3_types::metrics::MetricsName;
use g3_yaml::YamlDocPosition;

use super::{EscaperConfig, EscaperConfigDiffAction};
use crate::config::escaper::AnyEscaperConfig;

const ESCAPER_CONFIG_TYPE: &str = "ComplyAudit";

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ComplyAuditEscaperConfig {
    pub(crate) name: MetricsName,
    position: Option<YamlDocPosition>,
    pub(crate) next: MetricsName,
    pub(crate) auditor: MetricsName,
}

impl ComplyAuditEscaperConfig {
    pub(crate) fn new(position: Option<YamlDocPosition>) -> Self {
        ComplyAuditEscaperConfig {
            name: MetricsName::default(),
            position,
            next: MetricsName::default(),
            auditor: MetricsName::default(),
        }
    }

    pub(super) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut escaper = Self::new(position);
        g3_yaml::foreach_kv(map, |k, v| escaper.set(k, v))?;
        escaper.check()?;
        Ok(escaper)
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.next.is_empty() {
            return Err(anyhow!("next escaper is not set"));
        }
        if self.auditor.is_empty() {
            return Err(anyhow!("auditor is not set"));
        }
        Ok(())
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match k {
            super::CONFIG_KEY_ESCAPER_TYPE => Ok(()),
            super::CONFIG_KEY_ESCAPER_NAME => {
                self.name = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "next" => {
                self.next = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "auditor" => {
                self.auditor = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}

impl EscaperConfig for ComplyAuditEscaperConfig {
    fn name(&self) -> &MetricsName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn escaper_type(&self) -> &str {
        ESCAPER_CONFIG_TYPE
    }

    fn resolver(&self) -> &MetricsName {
        Default::default()
    }

    fn diff_action(&self, new: &AnyEscaperConfig) -> EscaperConfigDiffAction {
        let AnyEscaperConfig::ComplyAudit(new) = new else {
            return EscaperConfigDiffAction::SpawnNew;
        };

        if self.eq(new) {
            EscaperConfigDiffAction::NoAction
        } else {
            EscaperConfigDiffAction::Reload
        }
    }
}