use {
    crossterm::event::{
        KeyCode::{self, *},
        KeyEvent,
        KeyModifiers,
    },
};

macro_rules! const_key {
    ($name:ident, $code:expr) => {
        pub const $name: KeyEvent = KeyEvent {
            code: $code,
            modifiers: KeyModifiers::empty(),
        };
    };
    ($name:ident, $code:expr, $mod:expr) => {
        pub const $name: KeyEvent = KeyEvent {
            code: $code,
            modifiers: $mod,
        };
    };
}

// we define a few constants which make it easier to check key events
const_key!(ENTER, Enter);
const_key!(CONTROL_ENTER, Enter, KeyModifiers::CONTROL);
const_key!(ALT_ENTER, Enter, KeyModifiers::ALT);
//const_key!(BACKSPACE, Backspace);
//const_key!(BACK_TAB, BackTab, KeyModifiers::SHIFT); // backtab needs shift
//const_key!(DELETE, Delete);
const_key!(INSERT, Insert);
const_key!(DOWN, Down);
const_key!(PAGE_DOWN, PageDown);
const_key!(END, End);
const_key!(ESC, Esc);
const_key!(HOME, Home);
const_key!(LEFT, Left);
const_key!(QUESTION, Char('?'));
const_key!(SLASH, Char('/'));
const_key!(D, Char('d'));
const_key!(Y, Char('y'));
const_key!(N, Char('n'));
const_key!(SHIFT_QUESTION, Char('?'), KeyModifiers::SHIFT);
const_key!(RIGHT, Right);
//const_key!(SPACE, Char(' '));
const_key!(TAB, Tab);
const_key!(UP, Up);
const_key!(PAGE_UP, PageUp);
const_key!(CONTROL_C, Char('c'), KeyModifiers::CONTROL);
const_key!(CONTROL_H, Char('h'), KeyModifiers::CONTROL);
const_key!(CONTROL_F, Char('f'), KeyModifiers::CONTROL);
const_key!(CONTROL_N, Char('n'), KeyModifiers::CONTROL);
const_key!(CONTROL_O, Char('o'), KeyModifiers::CONTROL);
const_key!(CONTROL_Q, Char('q'), KeyModifiers::CONTROL);
const_key!(CONTROL_S, Char('s'), KeyModifiers::CONTROL);
const_key!(CONTROL_U, Char('u'), KeyModifiers::CONTROL);
const_key!(CONTROL_V, Char('v'), KeyModifiers::CONTROL);
const_key!(CONTROL_X, Char('x'), KeyModifiers::CONTROL);
const_key!(CONTROL_UP, Up, KeyModifiers::CONTROL);
const_key!(CONTROL_DOWN, Down, KeyModifiers::CONTROL);

/// return the raw char if the event is a letter event
pub fn as_letter(key: KeyEvent) -> Option<char> {
    match key {
        KeyEvent {
            code: KeyCode::Char(l),
            modifiers: KeyModifiers::NONE,
        } => Some(l),
        _ => None,
    }
}


/// build a human description of a key event
pub fn key_event_desc(key: KeyEvent) -> String {
    let mut s = String::new();
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        s.push('^');
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        s.push('⌥');
    }
    match key.code {
        Char('\r') | Char('\n') | Enter => {
            s.push('⏎');
        }
        Char(c) if key.modifiers.contains(KeyModifiers::SHIFT) => {
            s.push(c.to_ascii_uppercase());
        }
        Char(c) => {
            s.push(c.to_ascii_lowercase());
        }
        F(u) => {
            s.push_str(&format!("F{u}"));
        }
        _ => {
            s.push_str(&format!("{:?}", key.code));
        }
    }
    s
}
