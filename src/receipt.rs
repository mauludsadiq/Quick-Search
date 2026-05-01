use crate::hash::sha256_json;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunReceipt {
    pub receipt_schema: String,
    pub command: String,
    pub input: Value,
    pub output_digest: String,
    pub receipt_digest: String,
}

impl RunReceipt {
    pub fn new(command: &str, input: Value, output: &Value) -> Result<Self> {
        let output_digest = sha256_json(output)?;
        let mut r = Self {
            receipt_schema: "quick-search-receipt-v0.2.0".to_string(),
            command: command.to_string(),
            input,
            output_digest,
            receipt_digest: String::new(),
        };
        r.receipt_digest = sha256_json(&serde_json::json!({
            "receipt_schema": r.receipt_schema,
            "command": r.command,
            "input": r.input,
            "output_digest": r.output_digest
        }))?;
        Ok(r)
    }
}

pub fn with_receipt(command: &str, input: Value, output: Value) -> Result<Value> {
    let receipt = RunReceipt::new(command, input, &output)?;
    Ok(serde_json::json!({
        "output": output,
        "receipt": receipt
    }))
}
