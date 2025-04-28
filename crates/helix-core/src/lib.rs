#![deny(unsafe_code)]
#![warn(missing_docs)] // TODO: Enforce this later

//! Core Helix types, traits, and utilities shared across the platform.

// Core modules
pub mod agent;
/// Defines the Credential struct for secure storage.
pub mod credential;
pub mod errors;
pub mod event;
pub mod policy;
pub mod profile;
pub mod recipe;
pub mod state;
pub mod types;

pub use errors::HelixError;

/// Placeholder function to demonstrate module linkage.
pub fn hello_helix() -> String {
    "Hello from Helix Core!".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(hello_helix(), "Hello from Helix Core!");
    }
}
