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

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use smol_str::SmolStr;

use super::{ParseError, chars_allowed_in_opentsdb};

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct MetricTagName(SmolStr);

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct MetricTagValue(SmolStr);

pub type StaticMetricsTags = BTreeMap<MetricTagName, MetricTagValue>;

impl MetricTagName {
    /// # Safety
    /// The characters in `s` is not checked
    pub unsafe fn new_static_unchecked(s: &'static str) -> Self {
        MetricTagName(SmolStr::new_static(s))
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<str> for MetricTagName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for MetricTagName {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        chars_allowed_in_opentsdb(s)?;
        Ok(MetricTagName(s.into()))
    }
}

impl fmt::Display for MetricTagName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl MetricTagValue {
    pub const EMPTY: MetricTagValue = MetricTagValue(SmolStr::new_static(""));

    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<str> for MetricTagValue {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for MetricTagValue {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        chars_allowed_in_opentsdb(s)?;
        Ok(MetricTagValue(s.into()))
    }
}

impl fmt::Display for MetricTagValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_metrics_name() {
        assert_eq!(MetricTagName::from_str("abc-1").unwrap().as_str(), "abc-1");

        assert!(MetricTagName::from_str("a=b").is_err());
    }

    #[test]
    fn t_metrics_value() {
        assert_eq!(MetricTagValue::from_str("abc-1").unwrap().as_str(), "abc-1");

        assert!(MetricTagValue::from_str("a=b").is_err());
    }
}
