//! Protocol and frame handling for Portwell EZIO text displays.

use std::error::Error;
use std::fmt;

pub const WIDTH: usize = 16;
pub const HEIGHT: usize = 2;
pub const COMMAND_PREFIX: u8 = 0xfe;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    rows: [[u8; WIDTH]; HEIGHT],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameError {
    ControlCharacter { line: usize, character: char },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ControlCharacter { line, character } => write!(
                f,
                "line {line} contains unsupported control character U+{:04X}",
                *character as u32
            ),
        }
    }
}

impl Error for FrameError {}

impl Frame {
    pub fn new(line1: &str, line2: &str) -> Result<Self, FrameError> {
        Ok(Self {
            rows: [encode_line(1, line1)?, encode_line(2, line2)?],
        })
    }

    pub fn rows(&self) -> &[[u8; WIDTH]; HEIGHT] {
        &self.rows
    }
}

fn encode_line(line: usize, value: &str) -> Result<[u8; WIDTH], FrameError> {
    let mut output = [b' '; WIDTH];
    for (index, character) in value.chars().take(WIDTH).enumerate() {
        if character.is_control() {
            return Err(FrameError::ControlCharacter { line, character });
        }
        let byte = if character.is_ascii() {
            character as u8
        } else {
            b'?'
        };
        // 0xfe is the EZIO instruction prefix and must never be display data.
        output[index] = if byte == COMMAND_PREFIX { b'?' } else { byte };
    }
    Ok(output)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operation {
    Instruction(u8),
    Data(Vec<u8>),
}

pub struct EzioProtocol;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Button {
    Up,
    Down,
    Enter,
    Escape,
}

/// Converts a polled button level into one event per physical press.
#[derive(Default)]
pub struct ButtonDebouncer {
    pressed: Option<Button>,
}

impl ButtonDebouncer {
    pub fn update(&mut self, current: Option<Button>) -> Option<Button> {
        match (self.pressed, current) {
            (None, Some(button)) => {
                self.pressed = Some(button);
                Some(button)
            }
            (Some(previous), Some(button)) if previous != button => {
                self.pressed = Some(button);
                Some(button)
            }
            (_, None) => {
                self.pressed = None;
                None
            }
            _ => None,
        }
    }
}

impl fmt::Display for Button {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Enter => "enter",
            Self::Escape => "escape",
        };
        f.write_str(name)
    }
}

impl EzioProtocol {
    /// Initialization for an EZIO serial bridge with a two-line display.
    pub fn initialize() -> Vec<Operation> {
        vec![
            Operation::Instruction(0x28), // EZIO pre-init / 4-bit, two-line mode
            Operation::Instruction(0x28), // HD44780 function set
            Operation::Instruction(0x08), // display off during setup
            Operation::Instruction(0x01), // clear display
            Operation::Instruction(0x06), // left-to-right, no shift
            Operation::Instruction(0x02), // cursor home
            Operation::Instruction(0x0c), // display on, cursor off
        ]
    }

    pub fn write_frame(frame: &Frame) -> Vec<Operation> {
        vec![
            Operation::Instruction(0x80),
            Operation::Data(frame.rows[0].to_vec()),
            Operation::Instruction(0xc0),
            Operation::Data(frame.rows[1].to_vec()),
        ]
    }

    pub fn clear() -> Vec<Operation> {
        vec![Operation::Instruction(0x01)]
    }

    /// Decode the status byte returned after the EZIO read-key command.
    /// Both EZIO-100 (0x4x) and EZIO-300 (0xBx) encodings are accepted.
    pub fn decode_button(status: u8) -> Option<Button> {
        match status {
            0x4d | 0xbe => Some(Button::Up),
            0x47 | 0xbd => Some(Button::Down),
            0x4b | 0xbb => Some(Button::Enter),
            0x4e | 0xb7 => Some(Button::Escape),
            0x4f | 0xbf => None,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_pads_and_truncates_by_character() {
        let frame = Frame::new("short", "0123456789abcdefEXTRA").unwrap();
        assert_eq!(&frame.rows()[0], b"short           ");
        assert_eq!(&frame.rows()[1], b"0123456789abcdef");
    }

    #[test]
    fn frame_substitutes_non_ascii_without_splitting_utf8() {
        let frame = Frame::new("café", "🦀 rust").unwrap();
        assert_eq!(&frame.rows()[0][..4], b"caf?");
        assert_eq!(&frame.rows()[1][..6], b"? rust");
    }

    #[test]
    fn frame_rejects_control_characters() {
        assert!(matches!(
            Frame::new("bad\nline", "ok"),
            Err(FrameError::ControlCharacter { line: 1, .. })
        ));
    }

    #[test]
    fn frame_operations_address_both_rows() {
        let frame = Frame::new("one", "two").unwrap();
        let operations = EzioProtocol::write_frame(&frame);
        assert_eq!(operations[0], Operation::Instruction(0x80));
        assert_eq!(operations[2], Operation::Instruction(0xc0));
        assert_eq!(operations.len(), 4);
    }

    #[test]
    fn initialization_is_explicit_and_stable() {
        assert_eq!(
            EzioProtocol::initialize(),
            vec![0x28, 0x28, 0x08, 0x01, 0x06, 0x02, 0x0c]
                .into_iter()
                .map(Operation::Instruction)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn button_status_supports_both_ezio_generations() {
        assert_eq!(EzioProtocol::decode_button(0xbe), Some(Button::Up));
        assert_eq!(EzioProtocol::decode_button(0x4d), Some(Button::Up));
        assert_eq!(EzioProtocol::decode_button(0xbd), Some(Button::Down));
        assert_eq!(EzioProtocol::decode_button(0xbb), Some(Button::Enter));
        assert_eq!(EzioProtocol::decode_button(0xb7), Some(Button::Escape));
        assert_eq!(EzioProtocol::decode_button(0xbf), None);
        assert_eq!(EzioProtocol::decode_button(0x00), None);
    }

    #[test]
    fn button_debouncer_emits_one_event_until_release() {
        let mut debouncer = ButtonDebouncer::default();
        assert_eq!(debouncer.update(Some(Button::Up)), Some(Button::Up));
        assert_eq!(debouncer.update(Some(Button::Up)), None);
        assert_eq!(debouncer.update(None), None);
        assert_eq!(debouncer.update(Some(Button::Up)), Some(Button::Up));
    }
}
