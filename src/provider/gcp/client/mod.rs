//! GCP Secret Manager Client Implementations
//!
//! This module provides two implementations of the GCP Secret Manager client:
//! - **REST Client**: Native REST implementation using reqwest
//! - **gRPC Client**: Official Google Cloud SDK using gRPC

pub mod common;
pub mod grpc;
pub mod rest;

pub use grpc::SecretManagerGRPC;
pub use rest::SecretManagerREST;
