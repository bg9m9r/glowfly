//! The cursor at the right edge. After a write into the last column xterm keeps the cursor on
//! that cell with a pending wrap: the next character wraps, but a control that moves the cursor
//! or edits at it starts from the last cell and drops the pending wrap. vt100 keeps the cursor
//! one column past the edge instead, so those controls land one cell off: BS and CUB stop one
//! cell to the right, EL, ECH, ED, DCH and ICH miss the last cell, and a vertical move (LF,
//! CUU, CUD, VPA, RI) keeps the wrap pending on the new row. `Scan` follows the parser through
//! the output to find those controls, and `process` steps the cursor back onto the last cell
//! with a BS (which vt100 does without side effects) before each one that finds it past the
//! edge. Controls that leave the cursor alone (SGR, modes, titles) keep the wrap pending, as in
//! xterm. vt100 always wraps (it ignores DECAWM), so there is no unwrapped mode to follow.

/// Where the parser is in the output, as far as finding the controls above needs. Follows
/// vte's states: C0 controls run inside escape and control sequences, CAN and SUB cancel one,
/// and ESC starts a new one anywhere.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) enum Scan {
    #[default]
    Ground,
    /// After ESC; `inter` once an intermediate byte came.
    Esc { inter: bool },
    /// In a control sequence: `lead` is the first private-marker or intermediate byte (what
    /// vt100 dispatches on), `entry` until the first byte after `[`, and `ignore` once vte
    /// will drop the sequence.
    Csi { lead: Option<u8>, entry: bool, inter: bool, ignore: bool },
    /// In an OSC string (ends at BEL or ST) or a DCS, SOS, PM or APC string (ends at ST).
    Str { osc: bool },
}

impl Scan {
    /// Moves past `b`; true when `b` runs a control that starts from the cursor's cell.
    pub(super) fn step(&mut self, b: u8) -> bool {
        match b {
            0x18 | 0x1a => {
                *self = Scan::Ground;
                return false;
            }
            0x1b => {
                *self = Scan::Esc { inter: false };
                return false;
            }
            _ => {}
        }
        match *self {
            Scan::Str { osc } => {
                if osc && b == 0x07 {
                    *self = Scan::Ground;
                }
                false
            }
            _ if b < 0x20 => matches!(b, 0x08 | 0x0a..=0x0c),
            Scan::Ground => false,
            _ if b == 0x7f => false,
            Scan::Esc { inter } => {
                *self = match b {
                    0x20..=0x2f => Scan::Esc { inter: true },
                    b'[' if !inter => Scan::Csi { lead: None, entry: true, inter: false, ignore: false },
                    b']' if !inter => Scan::Str { osc: true },
                    b'P' | b'X' | b'^' | b'_' if !inter => Scan::Str { osc: false },
                    _ => {
                        *self = Scan::Ground;
                        // ESC M: reverse index.
                        return !inter && b == b'M';
                    }
                };
                false
            }
            Scan::Csi { lead, entry, inter, ignore } => {
                match b {
                    0x30..=0x3b => *self = Scan::Csi { lead, entry: false, inter, ignore: ignore || inter },
                    0x3c..=0x3f if entry => *self = Scan::Csi { lead: Some(b), entry: false, inter, ignore },
                    0x3c..=0x3f => *self = Scan::Csi { lead, entry: false, inter, ignore: true },
                    0x20..=0x2f => *self = Scan::Csi { lead: lead.or(Some(b)), entry: false, inter: true, ignore },
                    0x40..=0x7e => {
                        *self = Scan::Ground;
                        return !ignore && acts_at_cursor(lead, b);
                    }
                    _ => {}
                }
                false
            }
        }
    }
}

/// Control sequences (as vt100 dispatches them) that move from or edit at the cursor's cell:
/// ICH, CUU, CUD, CUB, ED, EL, DCH, ECH, VPA, and the selective ED and EL.
fn acts_at_cursor(lead: Option<u8>, fin: u8) -> bool {
    match lead {
        None => matches!(fin, b'@' | b'A' | b'B' | b'D' | b'J' | b'K' | b'P' | b'X' | b'd'),
        Some(b'?') => matches!(fin, b'J' | b'K'),
        Some(_) => false,
    }
}

/// Hands `bytes` to `parser`, first stepping the cursor back onto the last cell before each
/// control `scan` finds while the cursor is past the edge.
pub(super) fn process<CB: vt100::Callbacks>(parser: &mut vt100::Parser<CB>, scan: &mut Scan, bytes: &[u8]) {
    let mut done = 0;
    for (i, &b) in bytes.iter().enumerate() {
        if !scan.step(b) {
            continue;
        }
        if done < i {
            parser.process(&bytes[done..i]);
            done = i;
        }
        let screen = parser.screen();
        if screen.cursor_position().1 >= screen.size().1 {
            parser.process(b"\x08");
        }
    }
    parser.process(&bytes[done..]);
}
