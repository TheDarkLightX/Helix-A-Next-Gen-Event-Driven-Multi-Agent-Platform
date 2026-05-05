//! Minimal GUI preparation crate exposing an HTTP endpoint to list events.
//! This leverages Axum to serve JSON data that a future web-based GUI can consume.

use axum::{extract::State, routing::get, Json, Router};
use helix_core::event::Event;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Shared application state holding events to be displayed by the GUI.
#[derive(Clone)]
pub struct AppState {
    /// In-memory ring buffer of events collected from the Helix runtime.
    events: Arc<Mutex<VecDeque<Event>>>,
    capacity: usize,
}

impl AppState {
    /// Create a new [`AppState`] with the given ring buffer capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            events: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            capacity,
        }
    }

    /// Record a new event, dropping the oldest if capacity is exceeded.
    pub fn record_event(&self, event: Event) {
        let mut events = self.events.lock().expect("poisoned");
        if events.len() == self.capacity {
            events.pop_front();
        }
        events.push_back(event);
    }

    /// Return all currently stored events as a vector.
    fn all_events(&self) -> Vec<Event> {
        self.events
            .lock()
            .expect("poisoned")
            .iter()
            .cloned()
            .collect()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(100)
    }
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
    Router::new()
        .route("/events", get(list_events))
        .with_state(state)
}

async fn list_events(State(state): State<AppState>) -> Json<Vec<Event>> {
    Json(state.all_events())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use proptest::prelude::*;
    use serde_json::json;
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn list_events_returns_inserted_events() {
        let state = AppState::default();
        state.record_event(Event::new(
            "/test".into(),
            "type".into(),
            Some(json!({"k":"v"})),
        ));
        let router = app(state);
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/events")
                    .body(Body::empty())
                    .unwrap(),
            )
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
            for t in &types {
                state.record_event(Event::new("/src".into(), t.clone(), None));
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

    #[test]
    fn ring_buffer_drops_oldest_events() {
        let state = AppState::new(2);
        state.record_event(Event::new("/src".into(), "a".into(), None));
        state.record_event(Event::new("/src".into(), "b".into(), None));
        state.record_event(Event::new("/src".into(), "c".into(), None));
        let events = state.all_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].r#type, "b");
        assert_eq!(events[1].r#type, "c");
    }

    proptest! {
        #[test]
        fn prop_ring_buffer_keeps_latest(types in prop::collection::vec("[a-z]+".prop_map(|s| s.to_string()), 0..20)) {
            let cap = 5;
            let state = AppState::new(cap);
            for t in &types {
                state.record_event(Event::new("/src".into(), t.clone(), None));
            }
            let stored = state.all_events();
            let expected_len = std::cmp::min(types.len(), cap);
            prop_assert_eq!(stored.len(), expected_len);
            if types.len() > cap {
                let expected: Vec<_> = types[types.len()-cap..].to_vec();
                let stored_types: Vec<_> = stored.iter().map(|e| e.r#type.clone()).collect();
                prop_assert_eq!(stored_types, expected);
            }
        }
    }
}
