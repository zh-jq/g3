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

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use async_trait::async_trait;

use g3_types::metrics::NodeName;

use crate::config::resolver::AnyResolverConfig;

#[macro_use]
mod handle;
pub(crate) use handle::{
    ArcIntegratedResolverHandle, ArriveFirstResolveJob, HappyEyeballsResolveJob,
    IntegratedResolverHandle,
};
use handle::{BoxLoggedResolveJob, ErrorResolveJob, LoggedResolveJob};

mod stats;
pub(crate) use stats::ResolverStats;

mod registry;
pub(crate) use registry::{get_handle, get_names};

#[cfg(feature = "c-ares")]
mod c_ares;
mod hickory;

mod deny_all;
mod fail_over;

mod ops;
pub use ops::spawn_all;
pub(crate) use ops::{foreach_resolver, reload};

pub(crate) trait Resolver {
    fn get_handle(&self) -> ArcIntegratedResolverHandle;
    fn get_stats(&self) -> Arc<ResolverStats>;
}

#[async_trait]
trait ResolverInternal: Resolver {
    fn _dependent_resolver(&self) -> Option<BTreeSet<NodeName>>;

    fn _clone_config(&self) -> AnyResolverConfig;
    fn _update_config(
        &mut self,
        config: AnyResolverConfig,
        dep_table: BTreeMap<NodeName, ArcIntegratedResolverHandle>,
    ) -> anyhow::Result<()>;
    fn _update_dependent_handle(
        &mut self,
        target: &NodeName,
        handle: ArcIntegratedResolverHandle,
    ) -> anyhow::Result<()>;

    async fn _shutdown(&mut self);
}

type BoxResolverInternal = Box<dyn ResolverInternal + Send>;
