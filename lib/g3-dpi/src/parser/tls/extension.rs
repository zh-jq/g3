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

use thiserror::Error;

#[derive(Clone, Copy)]
#[repr(u16)]
pub enum ExtensionType {
    ServerName = 0,                           // rfc6066
    MaxFragmentLength = 1,                    // rfc6066
    StatusRequest = 5,                        // rfc6066
    SupportedGroups = 10,                     // rfc8422, rfc7919
    SignatureAlgorithms = 13,                 // rfc8446
    UseSrtp = 14,                             // rfc5764
    Heartbeat = 15,                           // rfc6520
    ApplicationLayerProtocolNegotiation = 16, // rfc7301
    SignedCertificateTimestamp = 18,          // rfc6962
    ClientCertificateType = 19,               // rfc7250
    ServerCertificateType = 20,               // rfc7250
    Padding = 21,                             // rfc7685
    PreSharedKey = 41,                        // rfc8446(TLS1.3)
    EarlyData = 42,                           // rfc8446(TLS1.3)
    SupportedVersions = 43,                   // rfc8446(TLS1.3)
    Cookie = 44,                              // rfc8446(TLS1.3)
    PskKeyExchangeModes = 45,                 // rfc8446(TLS1.3)
    CertificateAuthorities = 47,              // rfc8446(TLS1.3)
    OidFilters = 48,                          // rfc8446(TLS1.3)
    PostHandshakeAuth = 49,                   // rfc8446(TLS1.3)
    SignatureAlgorithmsCert = 50,             // rfc8446(TLS1.3)
    KeyShare = 51,                            // rfc8446(TLS1.3)
}

#[derive(Debug, Error)]
pub enum ExtensionParseError {
    #[error("not enough data")]
    NotEnoughData,
}

struct Extension<'a> {
    ext_type: u16,
    ext_len: u16,
    ext_data: Option<&'a [u8]>,
}

impl<'a> Extension<'a> {
    pub const HEADER_LEN: usize = 4;

    pub fn parse(data: &'a [u8]) -> Result<Self, ExtensionParseError> {
        if data.len() < Self::HEADER_LEN {
            return Err(ExtensionParseError::NotEnoughData);
        }

        let ext_type = u16::from_be_bytes([data[0], data[1]]);
        let ext_len = u16::from_be_bytes([data[2], data[3]]);

        if ext_len == 0 {
            Ok(Extension {
                ext_type,
                ext_len,
                ext_data: None,
            })
        } else {
            Ok(Extension {
                ext_type,
                ext_len,
                ext_data: Some(&data[Self::HEADER_LEN..Self::HEADER_LEN + ext_len as usize]),
            })
        }
    }
}

pub struct ExtensionList {}

impl ExtensionList {
    pub fn get_ext(
        data: &[u8],
        ext_type: ExtensionType,
    ) -> Result<Option<&[u8]>, ExtensionParseError> {
        let mut offset = 0usize;

        while offset < data.len() {
            let left = &data[offset..];
            let ext = Extension::parse(left)?;
            if ext.ext_type == ext_type as u16 {
                return Ok(ext.ext_data);
            }
            offset += Extension::HEADER_LEN + ext.ext_len as usize;
        }

        Ok(None)
    }
}
