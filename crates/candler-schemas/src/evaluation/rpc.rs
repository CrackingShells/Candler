//! The JSON-RPC 2.0 envelope of the evaluator protocol.
//!
//! Every line on an evaluator's stdin or stdout is one [`Message`]. The envelope adds `jsonrpc`
//! and, where JSON-RPC asks for it, `id` beside the `method`/`params` or `result`/`error` of the
//! roots in [`crate::evaluation`]; it does not restate them. Each message is read from an object
//! only, and a key the message does not define is refused.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::evaluation::{DescribeResult, EvaluateResult, Notification, Request};
use crate::map_only::MapOnly;
use crate::primitives::{RequestId, object_only, present};
use crate::values::JsonValue;

/// The only `jsonrpc` version the protocol speaks.
const JSONRPC_VERSION: &str = "2.0";

/// The `jsonrpc` member of a message, written as the string `2.0` and read from nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JsonRpcVersion;

impl Serialize for JsonRpcVersion {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(JSONRPC_VERSION)
    }
}

impl<'de> Deserialize<'de> for JsonRpcVersion {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        // BORROW: .as_str() compares the text with the version
        if text.as_str() == JSONRPC_VERSION {
            Ok(Self)
        } else {
            Err(serde::de::Error::custom(format!(
                "jsonrpc is not {JSONRPC_VERSION} (got {text})"
            )))
        }
    }
}

impl JsonSchema for JsonRpcVersion {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "JsonRpcVersion".into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "const": JSONRPC_VERSION,
        })
    }
}

/// A request from the core, written as an object with `jsonrpc`, an `id` and the `method` and `params` of a [`Request`].
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct RpcRequest {
    /// The protocol version, always `2.0`.
    pub jsonrpc: JsonRpcVersion,
    /// The id the response to this request carries.
    pub id: RequestId,
    /// The method and params of the request.
    #[serde(flatten)]
    pub call: Request,
}

/// The members of a request as they are read: the call is rebuilt from `method` and `params`, so a key the request does not define is refused (a flattened reader would let it pass).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRpcRequest {
    jsonrpc: JsonRpcVersion,
    id: RequestId,
    method: String,
    params: Value,
}

impl<'de> Deserialize<'de> for RpcRequest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let MapOnly(raw) = MapOnly::<RawRpcRequest>::deserialize(deserializer)?;
        let call = Request::deserialize(json!({ "method": raw.method, "params": raw.params }))
            .map_err(serde::de::Error::custom)?;
        Ok(Self {
            jsonrpc: raw.jsonrpc,
            id: raw.id,
            call,
        })
    }
}

impl RpcRequest {
    /// A request with its id.
    #[must_use]
    pub const fn new(id: RequestId, call: Request) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id,
            call,
        }
    }
}

/// A notification, written as an object with `jsonrpc` and the `method` and `params` of a [`Notification`], and no `id`.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct RpcNotification {
    /// The protocol version, always `2.0`.
    pub jsonrpc: JsonRpcVersion,
    /// The method and params of the notification.
    #[serde(flatten)]
    pub notification: Notification,
}

/// The members of a notification as they are read; see [`RawRpcRequest`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRpcNotification {
    jsonrpc: JsonRpcVersion,
    method: String,
    params: Value,
}

impl<'de> Deserialize<'de> for RpcNotification {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let MapOnly(raw) = MapOnly::<RawRpcNotification>::deserialize(deserializer)?;
        let notification =
            Notification::deserialize(json!({ "method": raw.method, "params": raw.params }))
                .map_err(serde::de::Error::custom)?;
        Ok(Self {
            jsonrpc: raw.jsonrpc,
            notification,
        })
    }
}

impl RpcNotification {
    /// A notification.
    #[must_use]
    pub const fn new(notification: Notification) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            notification,
        }
    }
}

/// The result a response carries: what an evaluator answers to `describe` or to `evaluate`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
#[non_exhaustive]
pub enum ResponseResult {
    /// The answer to a `describe` request.
    Describe(DescribeResult),
    /// The answer to an `evaluate` request.
    Evaluate(EvaluateResult),
}

/// A successful response, written as an object with `jsonrpc`, the `id` of the request and its `result`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self", deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct RpcResponse {
    /// The protocol version, always `2.0`.
    pub jsonrpc: JsonRpcVersion,
    /// The id of the request this answers.
    pub id: RequestId,
    /// What the evaluator answers.
    pub result: ResponseResult,
}

object_only!(RpcResponse);

impl RpcResponse {
    /// A response to the request with `id`.
    #[must_use]
    pub const fn new(id: RequestId, result: ResponseResult) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id,
            result,
        }
    }
}

/// The error of a failed request, written as an object with an integer `code`, a `message` and, when there is one, `data`; the code `-32800` is a request that was cancelled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self", deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct RpcError {
    /// The error code.
    pub code: i64,
    /// A sentence that says what went wrong.
    pub message: String,
    /// More about the error; absent when there is nothing more.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub data: Option<JsonValue>,
}

object_only!(RpcError);

impl RpcError {
    /// An error with a code and a message and no data.
    #[must_use]
    pub const fn new(code: i64, message: String) -> Self {
        Self {
            code,
            message,
            data: None,
        }
    }
}

/// A failed response, written as an object with `jsonrpc`, the `id` of the request and its `error`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self", deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct RpcErrorResponse {
    /// The protocol version, always `2.0`.
    pub jsonrpc: JsonRpcVersion,
    /// The id of the request this answers.
    pub id: RequestId,
    /// What went wrong.
    pub error: RpcError,
}

object_only!(RpcErrorResponse);

impl RpcErrorResponse {
    /// A failed response to the request with `id`.
    #[must_use]
    pub const fn new(id: RequestId, error: RpcError) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id,
            error,
        }
    }
}

/// One line of the evaluator protocol, written as an object of one of four shapes: a request, a notification, a response or an error response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
#[non_exhaustive]
pub enum Message {
    /// A request from the core.
    Request(RpcRequest),
    /// A notification, which has no id and gets no response.
    Notification(RpcNotification),
    /// A successful response.
    Response(RpcResponse),
    /// A failed response.
    Error(RpcErrorResponse),
}
