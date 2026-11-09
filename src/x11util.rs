//! Small X11 helpers built on x11rb (pure Rust, no libx11 needed).
//!
//! * `keysym_to_keycode` — find the keycode whose first keysym matches.
//! * `keysym_of_keycode` — reverse lookup (used by the settings UI when
//!   capturing a key).

use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt;

/// The keyboard mapping for a connection: keycode -> list of keysyms
/// (one per level/group). Fetched once at startup.
pub struct Keymap {
    min_keycode: u8,
    per_keycode: usize,
    keysyms: Vec<u32>,
}

impl Keymap {
    pub fn fetch<C: Connection>(conn: &C) -> Result<Self, Box<dyn std::error::Error>> {
        let setup = conn.setup();
        let min = setup.min_keycode;
        let max = setup.max_keycode;
        let count = (max - min + 1) as u8;
        let reply = conn.get_keyboard_mapping(min, count)?.reply()?;
        Ok(Keymap {
            min_keycode: min,
            per_keycode: reply.keysyms_per_keycode as usize,
            keysyms: reply.keysyms,
        })
    }

    fn keysyms_for(&self, keycode: u8) -> &[u32] {
        let idx = (keycode.wrapping_sub(self.min_keycode) as usize) * self.per_keycode;
        if idx >= self.keysyms.len() {
            return &[];
        }
        let end = (idx + self.per_keycode).min(self.keysyms.len());
        &self.keysyms[idx..end]
    }

    /// First keysym of a keycode (level 0, group 0), if any.
    pub fn first_keysym(&self, keycode: u8) -> Option<u32> {
        self.keysyms_for(keycode).first().copied().filter(|&k| k != 0)
    }

    /// Find the keycode whose FIRST keysym is `keysym`.
    pub fn keycode_for_keysym(&self, keysym: u32) -> Option<u8> {
        for kc in self.min_keycode..=u8::MAX {
            if self.first_keysym(kc) == Some(keysym) {
                return Some(kc);
            }
            if kc == u8::MAX {
                break;
            }
        }
        None
    }
}

/// Resolve a keysym NAME (e.g. "Control_R") to a keycode on this server.
pub fn keycode_for_name(
    keymap: &Keymap,
    name: &str,
) -> Option<u8> {
    let keysym = crate::keysym::keysym_from_name(name)?;
    keymap.keycode_for_keysym(keysym)
}

/// XTest extension present? (If not, clicks/scroll are disabled but
/// pointer movement still works.)
pub fn xtest_available<C: Connection>(conn: &C) -> bool {
    conn.extension_information("XTEST")
        .map(|info| info.is_some())
        .unwrap_or(false)
}
