//! Validating newtypes used across the Messages, Models, and Batches APIs.
//!
//! Each newtype enforces an invariant at construction so the rest of the
//! crate (and downstream callers) can rely on it without re-checking. The
//! `parse, don't validate` style means invalid values surface as a typed
//! error from `try_new` / `FromStr` rather than as opaque API failures
//! later.
//!
//! Numeric ranges:
//!
//! - [`MaxTokens`] — strictly positive; required by message creation.
//! - [`Temperature`], [`TopP`] — finite `0.0..=1.0` floats.
//! - [`TopK`] — strictly positive integer.
//!
//! Non-blank string identifiers:
//!
//! - [`ToolName`] — used by [`crate::Tool`] and tool-use blocks.
//! - [`ContainerId`], [`InferenceGeo`] — top-level Messages request fields.
//! - [`McpServerName`], [`McpServerUrl`] — MCP server definitions, with
//!   [`McpAuthorizationToken`] as a `Debug`-redacted bearer token.
//! - [`RequestId`] — Anthropic response header; also surfaced on
//!   [`crate::ApiErrorBody`].

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Maximum number of tokens the model may generate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaxTokens(u32);

impl MaxTokens {
    /// Creates a non-zero max token value.
    pub fn try_new(value: u32) -> Result<Self, MaxTokensError> {
        if value == 0 {
            return Err(MaxTokensError::Zero);
        }

        Ok(Self(value))
    }

    /// Returns the raw token count.
    pub fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for MaxTokens {
    type Error = MaxTokensError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<MaxTokens> for u32 {
    fn from(value: MaxTokens) -> Self {
        value.get()
    }
}

impl Serialize for MaxTokens {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> Deserialize<'de> for MaxTokens {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

/// Errors produced while constructing [`MaxTokens`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MaxTokensError {
    /// Token budgets must be greater than zero.
    #[error("max_tokens must be greater than zero")]
    Zero,
}

/// Anthropic request identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RequestId(String);

impl RequestId {
    /// Creates a request ID from a non-blank string.
    pub fn try_new(value: impl Into<String>) -> Result<Self, RequestIdError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(RequestIdError::Empty);
        }

        Ok(Self(value))
    }

    /// Returns the request ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts this request ID into its owned string.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<String> for RequestId {
    type Error = RequestIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl TryFrom<&str> for RequestId {
    type Error = RequestIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<RequestId> for String {
    fn from(value: RequestId) -> Self {
        value.into_string()
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RequestId {
    type Err = RequestIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

/// Errors produced while constructing [`RequestId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RequestIdError {
    /// Request IDs must not be blank.
    #[error("request ID must not be blank")]
    Empty,
}

/// Name of a caller-defined tool.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ToolName(String);

impl ToolName {
    /// Creates a tool name from a non-blank string.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ToolNameError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ToolNameError::Empty);
        }

        Ok(Self(value))
    }

    /// Returns the tool name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts this tool name into its owned string.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<String> for ToolName {
    type Error = ToolNameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl TryFrom<&str> for ToolName {
    type Error = ToolNameError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<ToolName> for String {
    fn from(value: ToolName) -> Self {
        value.into_string()
    }
}

impl fmt::Display for ToolName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ToolName {
    type Err = ToolNameError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

/// Errors produced while constructing [`ToolName`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ToolNameError {
    /// Tool names must not be blank.
    #[error("tool name must not be blank")]
    Empty,
}

/// Identifier for a reusable Messages API container.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContainerId(String);

impl ContainerId {
    /// Creates a container ID from a non-blank string.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ContainerIdError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ContainerIdError::Empty);
        }

        Ok(Self(value))
    }

    /// Returns the container ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts this container ID into its owned string.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<String> for ContainerId {
    type Error = ContainerIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl TryFrom<&str> for ContainerId {
    type Error = ContainerIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<ContainerId> for String {
    fn from(value: ContainerId) -> Self {
        value.into_string()
    }
}

impl fmt::Display for ContainerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ContainerId {
    type Err = ContainerIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

/// Errors produced while constructing [`ContainerId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ContainerIdError {
    /// Container IDs must not be blank.
    #[error("container ID must not be blank")]
    Empty,
}

/// Geographic region identifier for inference processing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct InferenceGeo(String);

impl InferenceGeo {
    /// Creates an inference geo from a non-blank string.
    pub fn try_new(value: impl Into<String>) -> Result<Self, InferenceGeoError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(InferenceGeoError::Empty);
        }

        Ok(Self(value))
    }

    /// Returns the inference geo as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts this inference geo into its owned string.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<String> for InferenceGeo {
    type Error = InferenceGeoError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl TryFrom<&str> for InferenceGeo {
    type Error = InferenceGeoError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<InferenceGeo> for String {
    fn from(value: InferenceGeo) -> Self {
        value.into_string()
    }
}

impl fmt::Display for InferenceGeo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for InferenceGeo {
    type Err = InferenceGeoError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

/// Errors produced while constructing [`InferenceGeo`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InferenceGeoError {
    /// Inference geo values must not be blank.
    #[error("inference_geo must not be blank")]
    Empty,
}

/// Temperature sampling value for compatibility requests.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Temperature(f64);

impl Temperature {
    /// Smallest accepted temperature value.
    pub const MIN: f64 = 0.0;
    /// Largest accepted temperature value.
    pub const MAX: f64 = 1.0;

    /// Creates a temperature value in the inclusive API range.
    pub fn try_new(value: f64) -> Result<Self, TemperatureError> {
        if !value.is_finite() {
            return Err(TemperatureError::NonFinite);
        }
        if !(Self::MIN..=Self::MAX).contains(&value) {
            return Err(TemperatureError::OutOfRange { actual: value });
        }

        Ok(Self(value))
    }

    /// Returns the raw temperature value.
    pub fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for Temperature {
    type Error = TemperatureError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<Temperature> for f64 {
    fn from(value: Temperature) -> Self {
        value.get()
    }
}

impl Serialize for Temperature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(self.0)
    }
}

impl<'de> Deserialize<'de> for Temperature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

/// Errors produced while constructing [`Temperature`].
#[derive(Debug, Clone, Copy, PartialEq, Error)]
pub enum TemperatureError {
    /// Temperature values must be finite JSON numbers.
    #[error("temperature must be a finite number")]
    NonFinite,
    /// Temperature values must be within the inclusive API range.
    #[error("temperature must be between 0.0 and 1.0 inclusive; got {actual}")]
    OutOfRange {
        /// Caller-provided temperature value.
        actual: f64,
    },
}

/// Positive `top_k` sampling value for compatibility requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TopK(u32);

impl TopK {
    /// Creates a positive top-k value.
    pub fn try_new(value: u32) -> Result<Self, TopKError> {
        if value == 0 {
            return Err(TopKError::Zero);
        }

        Ok(Self(value))
    }

    /// Returns the raw top-k value.
    pub fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for TopK {
    type Error = TopKError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<TopK> for u32 {
    fn from(value: TopK) -> Self {
        value.get()
    }
}

impl Serialize for TopK {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> Deserialize<'de> for TopK {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

/// Errors produced while constructing [`TopK`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TopKError {
    /// Top-k must be greater than zero.
    #[error("top_k must be greater than zero")]
    Zero,
}

/// Nucleus sampling value for compatibility requests.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct TopP(f64);

impl TopP {
    /// Smallest accepted top-p value.
    pub const MIN: f64 = 0.0;
    /// Largest accepted top-p value.
    pub const MAX: f64 = 1.0;

    /// Creates a top-p value in the inclusive API range.
    pub fn try_new(value: f64) -> Result<Self, TopPError> {
        if !value.is_finite() {
            return Err(TopPError::NonFinite);
        }
        if !(Self::MIN..=Self::MAX).contains(&value) {
            return Err(TopPError::OutOfRange { actual: value });
        }

        Ok(Self(value))
    }

    /// Returns the raw top-p value.
    pub fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for TopP {
    type Error = TopPError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<TopP> for f64 {
    fn from(value: TopP) -> Self {
        value.get()
    }
}

impl Serialize for TopP {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(self.0)
    }
}

impl<'de> Deserialize<'de> for TopP {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

/// Errors produced while constructing [`TopP`].
#[derive(Debug, Clone, Copy, PartialEq, Error)]
pub enum TopPError {
    /// Top-p values must be finite JSON numbers.
    #[error("top_p must be a finite number")]
    NonFinite,
    /// Top-p values must be within the inclusive API range.
    #[error("top_p must be between 0.0 and 1.0 inclusive; got {actual}")]
    OutOfRange {
        /// Caller-provided top-p value.
        actual: f64,
    },
}

/// Name of a request-scoped MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct McpServerName(String);

impl McpServerName {
    /// Creates an MCP server name from a non-blank string.
    pub fn try_new(value: impl Into<String>) -> Result<Self, McpServerNameError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(McpServerNameError::Empty);
        }

        Ok(Self(value))
    }

    /// Returns the MCP server name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts this MCP server name into its owned string.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<String> for McpServerName {
    type Error = McpServerNameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl TryFrom<&str> for McpServerName {
    type Error = McpServerNameError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<McpServerName> for String {
    fn from(value: McpServerName) -> Self {
        value.into_string()
    }
}

impl fmt::Display for McpServerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for McpServerName {
    type Err = McpServerNameError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

/// Errors produced while constructing [`McpServerName`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum McpServerNameError {
    /// MCP server names must not be blank.
    #[error("MCP server name must not be blank")]
    Empty,
}

/// URL of a request-scoped MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct McpServerUrl(String);

impl McpServerUrl {
    /// Creates an MCP server URL from a non-blank string.
    pub fn try_new(value: impl Into<String>) -> Result<Self, McpServerUrlError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(McpServerUrlError::Empty);
        }

        Ok(Self(value))
    }

    /// Returns the MCP server URL as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts this MCP server URL into its owned string.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<String> for McpServerUrl {
    type Error = McpServerUrlError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl TryFrom<&str> for McpServerUrl {
    type Error = McpServerUrlError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<McpServerUrl> for String {
    fn from(value: McpServerUrl) -> Self {
        value.into_string()
    }
}

impl fmt::Display for McpServerUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for McpServerUrl {
    type Err = McpServerUrlError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

/// Errors produced while constructing [`McpServerUrl`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum McpServerUrlError {
    /// MCP server URLs must not be blank.
    #[error("MCP server URL must not be blank")]
    Empty,
}

/// Authorization token for a request-scoped MCP server.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct McpAuthorizationToken(String);

impl McpAuthorizationToken {
    /// Creates an MCP authorization token from a non-blank string.
    pub fn try_new(value: impl Into<String>) -> Result<Self, McpAuthorizationTokenError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(McpAuthorizationTokenError::Empty);
        }

        Ok(Self(value))
    }

    /// Returns the authorization token as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts this authorization token into its owned string.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<String> for McpAuthorizationToken {
    type Error = McpAuthorizationTokenError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl TryFrom<&str> for McpAuthorizationToken {
    type Error = McpAuthorizationTokenError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<McpAuthorizationToken> for String {
    fn from(value: McpAuthorizationToken) -> Self {
        value.into_string()
    }
}

impl fmt::Debug for McpAuthorizationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

/// Errors produced while constructing [`McpAuthorizationToken`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum McpAuthorizationTokenError {
    /// MCP authorization tokens must not be blank.
    #[error("MCP authorization token must not be blank")]
    Empty,
}
