//! Minimal GUI preparation crate exposing an HTTP endpoint to list events.
//! This leverages Axum to serve JSON data that a future web-based GUI can consume.

use axum::{extract::State, routing::get, Json, Router};
use helix_core::event::Event;
use std::sync::{Arc, Mutex};

/// Shared application state holding events to be displayed by the GUI.
#[derive(Clone, Default)]
pub struct AppState {
    /// In-memory list of events collected from the Helix runtime.
    pub events: Arc<Mutex<Vec<Event>>>,
}

/// Build an Axum [`Router`] serving an `/events` endpoint returning all known events.
///
/// # Examples
///
/// ```
/// use helix_gui::{app, AppState};
/// let state = AppState::default();
/// let router = app(state);
/// ```
#[must_use]
pub fn app(state: AppState) -> Router {
    Router::new().route("/events", get(list_events)).with_state(state)
}

async fn list_events(State(state): State<AppState>) -> Json<Vec<Event>> {
    let events = state.events.lock().expect("poisoned").clone();
    Json(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::{Body, to_bytes}, http::Request};
    use proptest::prelude::*;
    use serde_json::json;
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn list_events_returns_inserted_events() {
        let state = AppState::default();
        {
            let mut events = state.events.lock().unwrap();
            events.push(Event::new("/test".into(), "type".into(), Some(json!({"k":"v"}))));
        }
        let router = app(state);
        let response = router
            .oneshot(Request::builder().uri("/events").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let events: Vec<Event> = serde_json::from_slice(&body).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].r#type, "type");
    }

    proptest! {
        #[test]
        fn prop_events_endpoint_len(types in prop::collection::vec("[a-z]+".prop_map(|s| s.to_string()), 0..5)) {
            let state = AppState::default();
            {
                let mut events = state.events.lock().unwrap();
                for t in &types {
                    events.push(Event::new("/src".into(), t.clone(), None));
                }
            }
            let router = app(state.clone());
            let rt = tokio::runtime::Runtime::new().unwrap();
            let response = rt.block_on(async {
                router
                    .oneshot(Request::builder().uri("/events").body(Body::empty()).unwrap())
                    .await
                    .unwrap()
            });
            let body = rt.block_on(async { to_bytes(response.into_body(), usize::MAX).await.unwrap() });
            let events: Vec<Event> = serde_json::from_slice(&body).unwrap();
            prop_assert_eq!(events.len(), types.len());
        }
    }
}
