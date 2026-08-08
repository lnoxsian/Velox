#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserState {
    Ground,
    Escape,
    CSI,
    OSC,
    EscapeDesignateG0,
    EscapeDesignateG1,
    EscapeDesignateG2,
    EscapeDesignateG3,
}

