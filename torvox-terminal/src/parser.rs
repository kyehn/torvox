use vte::Perform;

pub struct VtParser {
    parser: vte::Parser,
}

impl VtParser {
    pub fn new() -> Self {
        Self {
            parser: vte::Parser::new(),
        }
    }

    pub fn advance<P: Perform>(&mut self, handler: &mut P, bytes: &[u8]) {
        self.parser.advance(handler, bytes);
    }
}

impl Default for VtParser {
    fn default() -> Self {
        Self::new()
    }
}
