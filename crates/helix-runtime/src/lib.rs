#![warn(missing_docs)]

//! The core runtime engine for Helix, managing agent execution and event flow.

// TODO: Remove this placeholder function
/// Adds two numbers together (placeholder).
pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}

pub mod messaging;
