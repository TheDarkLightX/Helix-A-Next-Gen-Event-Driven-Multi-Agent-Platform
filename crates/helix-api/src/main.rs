// Copyright 2024 Helix Platform
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


//! Defines the external APIs (REST, gRPC) for Helix.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::Serialize;
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let app = app();

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// Define the health check response struct
#[derive(Serialize)]
struct HealthStatus {
    status: String,
}

// Define the health check handler
async fn health_check() -> impl IntoResponse {
    tracing::info!("Health check requested");
    let health = HealthStatus {
        status: "ok".to_string(),
    };
    (StatusCode::OK, Json(health))
}

// Separate function to create the Axum app (makes testing easier)
fn app() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .layer(TraceLayer::new_for_http())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_check() {
        let app = app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let body_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body_json, serde_json::json!({ "status": "ok" }));
    }
}
