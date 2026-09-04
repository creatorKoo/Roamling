// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Claude Code and Codex, as activity the pet can react to.
//!
//! Ported from Swift's `RoamlingSources`. This is the module that makes
//! Roamling the thing it says it is -- `docs/architecture.md` calls a coding
//! agent "one of the initial interest signals", and without this the pet
//! wanders and sleeps but never notices anyone working.
//!
//! Three pieces: the endpoint an agent's hook posts to (`receiver`), the
//! translation from its payload into a domain event (`normalize`), and writing
//! the hook into the user's own config (`installer`).
//!
//! **Product-specific payload does not leave this crate**, and the decode never
//! looks at prompt text, transcripts, tool input or output. That is the same
//! line the Swift module draws, drawn in the same place.

pub mod installer;
pub mod normalize;
pub mod receiver;

pub use normalize::{Agent, TOKEN_HEADER};
pub use receiver::Receiver;

/// The shared secret between an agent's hook and the loopback receiver.
///
/// Long enough that guessing is not the cheap way in, and stable once written:
/// the hook command in the user's config carries a copy, so a token that
/// changed on every launch would break every already-installed hook.
pub fn make_token() -> String {
    // Two sources of entropy that need no dependency: the clock, and the
    // address of a heap allocation. Sixty-four hex characters out.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let boxed = Box::new(0u8);
    let address = &*boxed as *const u8 as usize;
    let pid = std::process::id() as u128;

    let mut state = nanos ^ ((address as u128) << 32) ^ (pid << 96) ^ 0x9E37_79B9_7F4A_7C15;
    let mut token = String::with_capacity(64);
    for _ in 0..64 {
        // xorshift, which is plenty for a local secret nobody is grinding.
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        token.push(char::from_digit((state % 16) as u32, 16).unwrap_or('0'));
    }
    token
}

/// `roamling.claudeCodeHookToken` and `roamling.codexHookToken`, the same
/// settings keys the macOS runtime writes.
pub fn token_key(agent: Agent) -> &'static str {
    match agent {
        Agent::ClaudeCode => "roamling.claudeCodeHookToken",
        Agent::Codex => "roamling.codexHookToken",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The macOS side requires at least 24 characters before it will reuse a
    /// stored token, so anything shorter would be regenerated every launch and
    /// silently break every installed hook.
    #[test]
    fn a_token_is_long_and_not_the_same_twice() {
        let first = make_token();
        assert!(first.len() >= 24, "too short: {first}");
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(first, make_token());
    }
}
