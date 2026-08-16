#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum ParserState {
    Ground,
    Escape,
    CSI,
    OSC,
    DCS,
    EscapeDesignateG0,
    EscapeDesignateG1,
    EscapeDesignateG2,
    EscapeDesignateG3,
}
