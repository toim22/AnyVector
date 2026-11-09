//! AnyVector patch: a global hook that translates user interface text.
//!
//! The text of labels, buttons, menus, tooltips and window titles ([`crate::WidgetText`] made from
//! a string or a [`crate::RichText`]) and of [`crate::Painter::text`], [`crate::Painter::layout`]
//! and [`crate::Painter::layout_no_wrap`] goes through the translator before it is laid out.
//! A [`crate::text::LayoutJob`] is never translated, so the text a user types in a `TextEdit`
//! stays as typed.

use std::sync::RwLock;

/// Returns the translation of a text, or `None` to show it unchanged.
pub type Translator = fn(&str) -> Option<String>;

static TRANSLATOR: RwLock<Option<Translator>> = RwLock::new(None);

/// Set (or clear) the user interface translator.
pub fn set_text_translator(translator: Option<Translator>) {
    *TRANSLATOR.write().unwrap_or_else(|e| e.into_inner()) = translator;
}

/// The translation of `text`, or `text` itself.
pub(crate) fn translate(text: String) -> String {
    let translator = *TRANSLATOR.read().unwrap_or_else(|e| e.into_inner());
    match translator {
        Some(f) => f(&text).unwrap_or(text),
        None => text,
    }
}
