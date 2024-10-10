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

use anyhow::{anyhow, Context};
use rmpv::ValueRef;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};

fn as_certificates_from_single_element(
    value: &ValueRef,
) -> anyhow::Result<Vec<CertificateDer<'static>>> {
    let bytes = match value {
        ValueRef::String(s) => s.as_bytes(),
        ValueRef::Binary(b) => *b,
        _ => {
            return Err(anyhow!(
                "msgpack value type 'certificates' should be 'string' or 'binary'"
            ));
        }
    };
    let mut certs = Vec::new();
    for (i, r) in CertificateDer::pem_slice_iter(bytes).enumerate() {
        let cert = r.map_err(|e| anyhow!("invalid certificate #{i}: {e:?}"))?;
        certs.push(cert);
    }
    if certs.is_empty() {
        Err(anyhow!("no valid certificate found"))
    } else {
        Ok(certs)
    }
}

pub fn as_rustls_certificates(value: &ValueRef) -> anyhow::Result<Vec<CertificateDer<'static>>> {
    if let ValueRef::Array(seq) = value {
        let mut certs = Vec::with_capacity(seq.len());
        for (i, v) in seq.iter().enumerate() {
            let this_certs = as_certificates_from_single_element(v)
                .context(format!("invalid certificates value for element #{i}"))?;
            certs.extend(this_certs);
        }
        Ok(certs)
    } else {
        as_certificates_from_single_element(value)
    }
}

pub fn as_rustls_private_key(value: &ValueRef) -> anyhow::Result<PrivateKeyDer<'static>> {
    let bytes = match value {
        ValueRef::String(s) => s.as_bytes(),
        ValueRef::Binary(b) => *b,
        _ => {
            return Err(anyhow!(
                "msgpack value type for 'private key' should be 'string' or 'binary'"
            ));
        }
    };
    PrivateKeyDer::from_pem_slice(bytes).map_err(|e| anyhow!("invalid private key value: {e:?}"))
}
