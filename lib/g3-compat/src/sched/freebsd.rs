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

use std::io;
use std::mem;

#[derive(Clone)]
pub struct CpuAffinityImpl {
    cpu_set: libc::cpuset_t,
}

impl Default for CpuAffinityImpl {
    fn default() -> Self {
        CpuAffinityImpl {
            cpu_set: unsafe { mem::zeroed() },
        }
    }
}

pub const fn max_cpu_id() -> usize {
    let bytes = size_of::<libc::cpuset_t>();
    (bytes << 3) - 1
}

impl CpuAffinityImpl {
    pub const fn max_cpu_id(&self) -> usize {
        max_cpu_id()
    }

    pub(super) fn add_id(&mut self, id: usize) -> io::Result<()> {
        unsafe {
            libc::CPU_SET(id, &mut self.cpu_set);
        }
        Ok(())
    }

    pub(super) fn apply_to_local_thread(&self) -> io::Result<()> {
        let errno = unsafe {
            libc::cpuset_setaffinity(
                libc::CPU_LEVEL_WHICH,
                libc::CPU_WHICH_TID,
                -1,
                size_of::<libc::cpuset_t>() as libc::size_t,
                &self.cpu_set,
            )
        };
        if errno != 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}
