#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum ParserState {
    Ground,
    Escape,
    CSI,
    OSC,
    OscEscape,
    DCS,
    DcsEscape,
    EscapeDesignateG0,
    EscapeDesignateG1,
    EscapeDesignateG2,
    EscapeDesignateG3,
}
