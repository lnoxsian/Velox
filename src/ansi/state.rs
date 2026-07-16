#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserState {
    Ground,
    Escape,
    CSI,
    OSC,
    DCS,
    UTF8,
    Ignore,
    SOS,
    PM,
    APC,
}
