//! What a monitor says about ITSELF — the bytes it hands the machine
//! the moment it is plugged in, before any display server has an
//! opinion about it.
//!
//! Every screen carries a small block of description in its own
//! firmware: who made it, which model it is, its serial number, the
//! size of the picture in centimetres, and a handful of free-text
//! lines. The block is fetched by the kernel and laid out under
//! `/sys/class/drm/<card>-<connector>/edid`, one file per socket.
//!
//! THIS IS WHERE A SCREEN'S IDENTITY COMES FROM, and it is the whole
//! reason this module exists. The socket a monitor hangs off — `DP-1`,
//! `HDMI-A-1` — is a property of the CABLE, not of the screen: move the
//! plug one port along and every setting written under that name is
//! now describing a different monitor. What the firmware says moves
//! with the monitor, so it is the thing worth writing down.
//!
//! The byte offsets below are the ones the public VESA E-EDID
//! specification names — a header of eight fixed bytes, a
//! manufacturer's three letters packed five bits each, a product code,
//! a serial number, the picture size, and four eighteen-byte
//! descriptors. Read from the specification and from nothing else:
//! every program that reads EDID reads these same offsets because the
//! specification puts them there, which is a similarity the format
//! forces and not one borrowed from anybody's code.

/// The eight bytes every EDID block opens with. Not a checksum and not
/// a version: it is the one test that says "these bytes are a
/// description block" rather than a truncated read, an empty file or a
/// driver's placeholder.
const HEADER: [u8; 8] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];

/// One base block. Later blocks (extensions) carry modes and audio
/// capabilities and say nothing about identity, so they are not read.
const BLOCK: usize = 128;

/// Manufacturer, big-endian, three five-bit letters.
const MANUFACTURER: usize = 8;
/// Product code, little-endian.
const PRODUCT: usize = 10;
/// Serial number, little-endian.
const SERIAL: usize = 12;
/// Picture width and height in whole centimetres.
const CM_W: usize = 21;
const CM_H: usize = 22;

/// Where the four eighteen-byte descriptors start.
const DESCRIPTORS: [usize; 4] = [54, 72, 90, 108];
const DESCRIPTOR: usize = 18;
/// The descriptor tags that carry text worth having: the serial number
/// as the maker prints it on the back, and the name the monitor calls
/// itself.
const TAG_SERIAL_TEXT: u8 = 0xFF;
const TAG_NAME: u8 = 0xFC;

/// How much of a text serial number becomes part of a key. Long enough
/// for every serial anybody prints on a label, short enough that a
/// monitor whose firmware pads the field with junk cannot make a key
/// nobody can read or type.
const SERIAL_TEXT_MAX: usize = 16;

/// What one monitor says it is.
///
/// Every field is what the block held, unjudged — the judging is in
/// [`Edid::identity`], which is the only method that decides whether
/// what was said is enough to name a screen by.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Edid {
    /// The maker's three letters — `DEL`, `SAM`, `GSM`. `None` when
    /// the five-bit letters are not letters, which is a firmware that
    /// has answered nonsense rather than a monitor without a maker.
    pub manufacturer: Option<String>,
    /// The model number the maker gave this panel. Every unit of one
    /// model carries the same one, which is exactly why it is not an
    /// identity on its own.
    pub product: u16,
    /// The serial number as a number. Zero is the customary "not
    /// given", and a great many monitors give it.
    pub serial: u32,
    /// The serial number as TEXT, from a descriptor. Where the number
    /// above is zero this is often where the real one is, so the two
    /// are one question asked twice — see [`Edid::serial_key`].
    pub serial_text: Option<String>,
    /// What the monitor calls itself: `U2720Q`, `LG ULTRAWIDE`. A
    /// LABEL and never a key — every unit of a model says the same
    /// thing.
    pub name: Option<String>,
    /// Picture size in whole centimetres, as the block gives it. Zero
    /// is "not said": projectors and anything variable report it.
    pub cm_w: u8,
    pub cm_h: u8,
}

/// The identity fields of an EDID block, or `None` when the bytes are
/// not an EDID block at all.
///
/// The header is required and the checksum is not, and that split is
/// deliberate. The header answers "are these description bytes"; a
/// checksum answers "did the firmware's author add up correctly", and
/// monitors that get it wrong are common enough that refusing them
/// would cost their owners the identity this whole module exists to
/// give.
///
/// WHAT THAT COSTS, plainly, because the next reader will weigh the
/// same trade: of the three fields a key is built from, only the
/// maker's is checked at all — its letters are five-bit numbers with a
/// range, so nonsense there yields no identity ([`Edid::identity`]).
/// The product code and the serial number are sixteen and thirty-two
/// bits of anything, and a flipped bit in either produces a
/// well-shaped key that simply matches nothing in the file: that
/// screen quietly takes the default settings until the byte reads
/// right again. What it CANNOT do is point at another screen — a wrong
/// key is a key nothing answers to, not one somebody else answers to —
/// and a checksum would turn the same flipped bit into no key at all,
/// which costs that screen exactly as much.
pub fn parse(bytes: &[u8]) -> Option<Edid> {
    if bytes.len() < BLOCK || bytes[..HEADER.len()] != HEADER {
        return None;
    }
    Some(Edid {
        manufacturer: letters(bytes[MANUFACTURER], bytes[MANUFACTURER + 1]),
        product: u16::from_le_bytes([bytes[PRODUCT], bytes[PRODUCT + 1]]),
        serial: u32::from_le_bytes([
            bytes[SERIAL],
            bytes[SERIAL + 1],
            bytes[SERIAL + 2],
            bytes[SERIAL + 3],
        ]),
        serial_text: descriptor_text(bytes, TAG_SERIAL_TEXT),
        name: descriptor_text(bytes, TAG_NAME),
        cm_w: bytes[CM_W],
        cm_h: bytes[CM_H],
    })
}

/// The block the kernel published for one connector, if there is one.
///
/// The directory under `/sys/class/drm` is named `<card>-<connector>`
/// and which card a monitor hangs off is not knowledge this program
/// has, so the connector is matched as a SUFFIX. The leading dash is
/// what keeps `DP-1` off `eDP-1`'s file.
///
/// WHICH IS AMBIGUOUS ON A MACHINE WITH TWO GRAPHICS CARDS: `card0-DP-1`
/// and `card1-DP-1` are two sockets with one name, and the display
/// server named only one of them. Taking whichever the directory listing
/// happened to hand over first would be a coin toss for the name every
/// setting of that screen is written under, so all of them are read and
/// they have to AGREE ([`agreed`]). When they do not, this screen has no
/// identity worth the risk and takes the socket's name instead — the
/// answer it had before monitors were read at all.
pub fn read(connector: &str) -> Option<Edid> {
    let suffix = format!("-{connector}");
    let blocks: Vec<Edid> = std::fs::read_dir("/sys/class/drm")
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name().map(|n| n.to_string_lossy().ends_with(&suffix)).unwrap_or(false)
        })
        .filter_map(|p| std::fs::read(p.join("edid")).ok().as_deref().and_then(parse))
        .collect();
    let found = agreed(&blocks).cloned();
    if found.is_none() && !blocks.is_empty() {
        eprintln!(
            "nacelle-desktop: more than one card publishes a monitor for '{connector}' and \
             they are different monitors \u{2014} that screen is named by its socket"
        );
    }
    found
}

/// The one monitor a set of candidate blocks agrees on: the first of
/// them when they all name the same screen, and nothing when they do
/// not.
///
/// Identity and not the whole block, because that is the thing being
/// decided — two readings of one monitor may differ in a descriptor
/// this program does not use, and neither of those readings is the
/// wrong screen.
fn agreed(blocks: &[Edid]) -> Option<&Edid> {
    let first = blocks.first()?;
    blocks
        .iter()
        .all(|b| b.identity() == first.identity())
        .then_some(first)
}

impl Edid {
    /// What this monitor is called in a configuration file — the body
    /// of an identity key, without the prefix that marks it as one.
    ///
    /// Maker, model and serial, in that order and no more: `DEL-A1B2-
    /// 0123ABCD`. The monitor's own NAME is deliberately absent, being
    /// the same string on every unit of the model, and so is the
    /// connector, being a property of the cable.
    ///
    /// A monitor that gives no serial at all — neither a number nor a
    /// descriptor — still gets a key, one part shorter. It is worth
    /// having: it survives the replug and the power-on order, which is
    /// the whole point, and the ONE thing it cannot do is tell two
    /// units of the same model apart. Those two are indistinguishable
    /// to anything reading their firmware, so they share an
    /// assignment; an owner who needs them apart writes a key naming
    /// the SOCKET instead, which goes on working beside this one.
    pub fn identity(&self) -> Option<String> {
        let mfg = self.manufacturer.as_deref()?;
        let mut key = format!("{mfg}-{:04X}", self.product);
        if let Some(serial) = self.serial_key() {
            key.push('-');
            key.push_str(&serial);
        }
        Some(key)
    }

    /// The serial number as a key part: the number when there is one,
    /// the printed text when there is not, and nothing when neither.
    ///
    /// The text is reduced to letters and digits because it is going
    /// into a key somebody may have to type, and firmware puts spaces,
    /// dashes and trailing padding in that field freely.
    fn serial_key(&self) -> Option<String> {
        if self.serial != 0 {
            return Some(format!("{:08X}", self.serial));
        }
        let cleaned: String = self
            .serial_text
            .as_deref()?
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_uppercase())
            .take(SERIAL_TEXT_MAX)
            .collect();
        (!cleaned.is_empty()).then_some(cleaned)
    }

    /// What to CALL this screen where a person will read it: the name
    /// the monitor gives itself, and failing that the maker and model
    /// it cannot avoid giving.
    pub fn model(&self) -> Option<String> {
        if let Some(name) = &self.name {
            return Some(name.clone());
        }
        Some(format!("{} {:04X}", self.manufacturer.as_deref()?, self.product))
    }

    /// The picture's diagonal in inches, from the centimetres in the
    /// block. `None` when either dimension is zero — a projector, a
    /// headless output, a firmware that keeps quiet: no measurement,
    /// never a guess.
    pub fn diagonal_in(&self) -> Option<f32> {
        let (w, h) = (self.cm_w as f32, self.cm_h as f32);
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        Some((w * w + h * h).sqrt() / 2.54)
    }
}

/// The maker's three letters out of two bytes: a big-endian word
/// holding three five-bit numbers, 1 through 26 for A through Z.
///
/// Anything outside that range is not a letter, and one bad letter
/// spoils the whole word: what comes back is a maker's code or
/// nothing, never two thirds of one.
fn letters(hi: u8, lo: u8) -> Option<String> {
    let packed = u16::from_be_bytes([hi, lo]);
    let mut out = String::with_capacity(3);
    for shift in [10, 5, 0] {
        let n = ((packed >> shift) & 0x1F) as u8;
        if !(1..=26).contains(&n) {
            return None;
        }
        out.push((b'A' + n - 1) as char);
    }
    Some(out)
}

/// The text of the first descriptor carrying one tag.
///
/// A descriptor is a DISPLAY descriptor — text rather than a video
/// mode — when its first three bytes and its fifth are zero; the
/// fourth is then the tag and the rest is thirteen bytes of text,
/// ended by a line feed and padded with spaces.
fn descriptor_text(bytes: &[u8], tag: u8) -> Option<String> {
    for at in DESCRIPTORS {
        let d = bytes.get(at..at + DESCRIPTOR)?;
        if d[0] != 0 || d[1] != 0 || d[2] != 0 || d[4] != 0 || d[3] != tag {
            continue;
        }
        let text: String = d[5..DESCRIPTOR]
            .iter()
            .copied()
            .take_while(|&b| b != 0x0A)
            // Firmware puts control bytes and high bytes in this field;
            // what is wanted is the part a person could read.
            .filter(|b| (0x20..0x7F).contains(b))
            .map(|b| b as char)
            .collect();
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A block built to order, so every test below says what it is
    /// about rather than pointing at a wall of hex.
    ///
    /// `pub(crate)` because the screens and the configuration are
    /// tested against monitors too, and a second builder beside this
    /// one would be a second opinion about what an EDID looks like.
    pub(crate) fn block(
        maker: &str,
        product: u16,
        serial: u32,
        cm: (u8, u8),
        name: Option<&str>,
        serial_text: Option<&str>,
    ) -> Vec<u8> {
        let mut b = vec![0u8; BLOCK];
        b[..HEADER.len()].copy_from_slice(&HEADER);
        let packed = maker
            .bytes()
            .take(3)
            .fold(0u16, |acc, c| (acc << 5) | u16::from(c.to_ascii_uppercase() - b'A' + 1));
        b[MANUFACTURER..MANUFACTURER + 2].copy_from_slice(&packed.to_be_bytes());
        b[PRODUCT..PRODUCT + 2].copy_from_slice(&product.to_le_bytes());
        b[SERIAL..SERIAL + 4].copy_from_slice(&serial.to_le_bytes());
        b[CM_W] = cm.0;
        b[CM_H] = cm.1;
        for (at, (tag, text)) in DESCRIPTORS
            .iter()
            .skip(1)
            .zip([(TAG_NAME, name), (TAG_SERIAL_TEXT, serial_text)])
        {
            let Some(text) = text else { continue };
            b[at + 3] = tag;
            let bytes = text.as_bytes();
            b[at + 5..at + 5 + bytes.len()].copy_from_slice(bytes);
            if at + 5 + bytes.len() < at + DESCRIPTOR {
                b[at + 5 + bytes.len()] = 0x0A;
            }
        }
        b
    }

    /// The whole of what a monitor says, out of the bytes it says it
    /// in. Dell's registered code really is `DEL` = 0x10AC, so the
    /// packing below is checked against a number from outside this
    /// program rather than against itself.
    #[test]
    fn a_monitor_says_who_made_it_which_model_it_is_and_which_unit() {
        let bytes = block("DEL", 0x41B2, 0x0123ABCD, (60, 34), Some("U2720Q"), Some("7MT0183"));
        assert_eq!(bytes[MANUFACTURER], 0x10, "DEL packs to 0x10AC");
        assert_eq!(bytes[MANUFACTURER + 1], 0xAC);
        let e = parse(&bytes).expect("a header and a full block is an EDID");
        assert_eq!(e.manufacturer.as_deref(), Some("DEL"));
        assert_eq!(e.product, 0x41B2);
        assert_eq!(e.serial, 0x0123ABCD);
        assert_eq!(e.name.as_deref(), Some("U2720Q"));
        assert_eq!(e.serial_text.as_deref(), Some("7MT0183"));
        assert_eq!(e.identity().as_deref(), Some("DEL-41B2-0123ABCD"));
        assert_eq!(e.model().as_deref(), Some("U2720Q"), "the label is the monitor's own name");
        let d = e.diagonal_in().expect("60x34 cm is a measurement");
        assert!((d - 27.1).abs() < 0.2, "60x34 cm is a 27\" panel, got {d}");
    }

    /// Bytes that are not a description block name nothing. The old
    /// reading took byte 21 from whatever it was handed, which on a
    /// short read or a driver's placeholder is a number with no
    /// meaning at all.
    #[test]
    fn bytes_that_are_not_an_edid_describe_no_monitor() {
        assert!(parse(&[]).is_none(), "an empty file is not a monitor");
        assert!(parse(&vec![0u8; BLOCK]).is_none(), "zeroes are not a header");
        let short = block("DEL", 1, 1, (60, 34), None, None);
        assert!(parse(&short[..64]).is_none(), "half a block is not a block");
        // A block whose maker's letters are not letters: the size is
        // still a size, but there is no key to be built from it.
        let mut wrong = block("DEL", 1, 1, (60, 34), None, None);
        wrong[MANUFACTURER] = 0x00;
        wrong[MANUFACTURER + 1] = 0x00;
        let e = parse(&wrong).expect("the block is still a block");
        assert!(e.manufacturer.is_none(), "0 is not a letter");
        assert!(e.identity().is_none(), "and no key is built from nonsense");
        assert!(e.diagonal_in().is_some(), "while the measurement is untouched");
    }

    /// The serial number is one question the block answers in two
    /// places, and a key has to take whichever answer exists.
    #[test]
    fn the_serial_is_the_number_or_the_printed_text_or_neither() {
        let numbered = parse(&block("SAM", 0x0F, 0x00BEEF01, (60, 34), None, Some("SN-9"))).unwrap();
        assert_eq!(
            numbered.identity().as_deref(),
            Some("SAM-000F-00BEEF01"),
            "a number that is there is the answer"
        );
        let printed = parse(&block("SAM", 0x0F, 0, (60, 34), None, Some("sn 9/x"))).unwrap();
        assert_eq!(
            printed.identity().as_deref(),
            Some("SAM-000F-SN9X"),
            "a zero number hands the question to the printed one, letters and digits only"
        );
        let silent = parse(&block("SAM", 0x0F, 0, (60, 34), None, None)).unwrap();
        assert_eq!(
            silent.identity().as_deref(),
            Some("SAM-000F"),
            "a monitor that gives no serial is still named, one part shorter"
        );
        // Padding is not a serial number.
        let padded = parse(&block("SAM", 0x0F, 0, (60, 34), None, Some("   "))).unwrap();
        assert_eq!(padded.identity().as_deref(), Some("SAM-000F"));
        // Two units of that same silent model are one key. Said out
        // loud here because it is the cost of the line above, and the
        // socket is what still tells them apart.
        let twin = parse(&block("SAM", 0x0F, 0, (60, 34), None, None)).unwrap();
        assert_eq!(silent.identity(), twin.identity());
    }

    /// A monitor that measures nothing is not a small monitor.
    #[test]
    fn a_silent_size_is_no_size() {
        let e = parse(&block("ACR", 1, 1, (0, 34), None, None)).unwrap();
        assert!(e.diagonal_in().is_none());
        let e = parse(&block("ACR", 1, 1, (60, 0), None, None)).unwrap();
        assert!(e.diagonal_in().is_none());
    }

    /// Two cards, one connector name. The kernel's directory listing
    /// decides nothing here: either the readings name one screen or
    /// this program does not know which screen it is looking at.
    #[test]
    fn a_connector_two_cards_answer_for_is_named_only_when_they_agree() {
        let dell = parse(&block("DEL", 0x41B2, 0x0123ABCD, (60, 34), None, None)).unwrap();
        let lg = parse(&block("GSM", 0x5B0F, 0x00000007, (60, 34), None, None)).unwrap();

        assert!(agreed(&[]).is_none(), "no card answered at all");
        assert_eq!(agreed(std::slice::from_ref(&dell)), Some(&dell), "one answer is the answer");
        assert_eq!(
            agreed(&[dell.clone(), dell.clone()]),
            Some(&dell),
            "the same monitor read twice is still that monitor"
        );
        assert!(
            agreed(&[dell.clone(), lg.clone()]).is_none(),
            "two different monitors on one connector name: no identity is worth the guess"
        );
        assert!(agreed(&[lg, dell]).is_none(), "and the listing's order decides nothing");
    }

    /// With no name descriptor the label falls back to what the block
    /// cannot avoid carrying — and never to nothing, because a screen
    /// the user has to pick out of a list needs SOMETHING written on it.
    #[test]
    fn a_monitor_without_a_name_is_still_labelled() {
        let e = parse(&block("GSM", 0x5B0F, 7, (60, 34), None, None)).unwrap();
        assert_eq!(e.model().as_deref(), Some("GSM 5B0F"));
    }
}
