// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::focus_activity::FocusActivity;
use std::sync::Mutex;

// --------------------------------------------------------- the app in front

/// The desk itself as a state source: which app is in front and how long since
/// a keystroke, translated into declarations for the activity director.
///
/// The shell samples; every judgement about what that sample means is inside.
#[derive(uniffi::Object)]
pub struct FocusWatch {
    inner: Mutex<FocusActivity>,
}

#[uniffi::export]
impl FocusWatch {
    #[uniffi::constructor]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            inner: Mutex::new(FocusActivity::new()),
        })
    }

    /// `app_id` is nil when this app itself is in front, or when the platform
    /// cannot name what is -- which means "unknown", never "the user left".
    pub fn observe(
        &self,
        app_id: Option<String>,
        watched: bool,
        seconds_since_key: f64,
        now: f64,
    ) -> Vec<FfiStateDeclaration> {
        self.inner
            .lock()
            .unwrap()
            .observe(app_id.as_deref(), watched, seconds_since_key, now)
            .into_iter()
            .map(FfiStateDeclaration::from)
            .collect()
    }

    /// The apps the menu offers, most recent first.
    pub fn recent_apps(&self) -> Vec<String> {
        self.inner.lock().unwrap().recent_apps()
    }
}
