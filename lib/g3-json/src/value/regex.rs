/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use regex::Regex;
use serde_json::Value;

pub fn as_regex(value: &Value) -> anyhow::Result<Regex> {
    if let Value::String(s) = value {
        let regex = Regex::new(s).map_err(|e| anyhow!("invalid regex value: {e}"))?;
        Ok(regex)
    } else {
        Err(anyhow!(
            "the yaml value type for regex string should be 'string'"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_regex_ok() {
        // valid regex string
        let value = Value::String("^\\d{3}-\\d{2}-\\d{4}$".to_string());
        assert_eq!(as_regex(&value).unwrap().as_str(), "^\\d{3}-\\d{2}-\\d{4}$");

        let value = Value::String("^[a-zA-Z]+$".to_string());
        assert_eq!(as_regex(&value).unwrap().as_str(), "^[a-zA-Z]+$");
    }

    #[test]
    fn as_regex_err() {
        // invalid regex string
        let value = Value::String("^\\d{3-\\d{2}-\\d{4}$".to_string());
        assert!(as_regex(&value).is_err());

        // non-string type
        let value = Value::Number(123.into());
        assert!(as_regex(&value).is_err());
    }
}
