use anyhow::{Ok, Result};
use std::collections::HashSet;

use crate::parsers::ParseError;

#[derive(Clone)]
pub struct Block {
    pub filename: String,
    pub line_number: usize,
    pub text: String,
    pub subblocks: Vec<Block>,
}

pub struct Lexer {
    block: Vec<Block>,
    init: bool,
    eob: bool,
    line: usize,
    filename: String,
    line_number: usize,
    text: String,
    subblock: Vec<Block>,
    pos: usize,
    keywords: HashSet<&'static str>,
}

#[derive(Clone)]
pub struct LexerState {
    filename: String,
    line_number: usize,
    text: String,
    subblock: Vec<Block>,
    pos: usize,
}

impl Lexer {
    pub fn new(block: Vec<Block>, init: bool) -> Self {
        let keywords = HashSet::from(["hide", "init", "jump", "return", "scene", "show"]);

        Lexer {
            block,
            init,
            eob: false,
            line: 0,
            filename: String::new(),
            line_number: 0,
            text: String::new(),
            subblock: Vec::new(),
            pos: 0,
            keywords,
        }
    }

    pub fn eob(&self) -> bool {
        return self.eob;
    }

    pub fn text(&self) -> String {
        return self.text.clone();
    }

    pub fn advance(&mut self) -> bool {
        self.line += 1;

        if self.line >= self.block.len() {
            self.eob = true;
            return false;
        }

        let block = self.block[self.line].clone();

        self.filename = block.filename;
        self.line_number = block.line_number;
        self.text = block.text;
        self.subblock = block.subblocks;
        self.pos = 0;

        true
    }

    fn match_regexp(&mut self, regexp: &str) -> Option<String> {
        if self.eob {
            return None;
        }

        if self.pos == self.text.len() {
            return None;
        }

        let re = regex::Regex::new(regexp).unwrap();
        let m = re.find(&self.text[self.pos..]);

        if let Some(m) = m {
            self.pos += m.end();
            Some(self.text[m.start()..m.end()].to_string())
        } else {
            None
        }
    }

    fn skip_whitespace(&mut self) {
        self.match_regexp(r"\s+");
    }

    pub fn match_(&mut self, regexp: &str) -> Option<String> {
        self.skip_whitespace();
        self.match_regexp(regexp)
    }

    pub fn keyword(&mut self, regexp: &str) -> Option<String> {
        self.match_(&format!(r"{}\b", regexp))
    }

    pub fn error(&self, msg: &str) -> Result<()> {
        let err = ParseError {
            filename: self.filename.clone(),
            line_number: self.line_number,
            message: msg.to_string(),
            line: Some(self.text.clone()),
            pos: Some(self.pos),
        }
        .into();
        Err(err)
    }

    pub fn eol(&mut self) -> bool {
        self.skip_whitespace();
        self.pos >= self.text.len()
    }

    pub fn expect_eol(&mut self) -> Result<()> {
        if !self.eol() {
            self.error("end of line expected")?;
        }
        Ok(())
    }

    pub fn expect_noblock(&mut self, stmt: &str) -> Result<()> {
        if !self.subblock.is_empty() {
            self.error(&format!("{} does not expect a block. Please check the indentation of the line after this one.", stmt))?;
        }
        Ok(())
    }

    pub fn expect_block(&mut self, stmt: &str) -> Result<()> {
        if self.subblock.is_empty() {
            self.error(&format!("{} expects a non-empty block.", stmt))?;
        }
        Ok(())
    }

    pub fn subblock_lexer(&mut self, init: bool) -> Lexer {
        Lexer::new(self.subblock.clone(), self.init || init)
    }

    pub fn string(&mut self) -> Option<String> {
        let s = self
            .match_(r#"r?"([^\\"]|\\.)*""#)
            .or_else(|| self.match_(r#"r?'([^\\']|\\.)*'"#))
            .or_else(|| self.match_(r#"r?`([^\\`]|\\.)*`"#));

        if let Some(s) = s {
            let mut s: String = s[1..s.len() - 1].to_string();
            if s.starts_with('r') {
                s = s[1..].to_string();
            } else {
                s = s.replace("\\n", "\n");
                s = regex::Regex::new(r"\\u([0-9a-fA-F]{1,4})")
                    .unwrap()
                    .replace_all(&s, |caps: &regex::Captures| {
                        let hex = &caps[1];
                        String::from_utf8(vec![u8::from_str_radix(hex, 16).unwrap()]).unwrap()
                    })
                    .to_string();

                s = regex::Regex::new(r"\\.")
                    .unwrap()
                    .replace_all(&s, |caps: &regex::Captures| {
                        caps.get(0).unwrap().as_str()[1..].to_string()
                    })
                    .to_string();

                s = regex::Regex::new(r"\s+")
                    .unwrap()
                    .replace_all(&s, " ")
                    .to_string();
            }
            Some(s.to_string())
        } else {
            None
        }
    }

    pub fn integer(&mut self) -> Option<String> {
        self.match_(r"(\+|\-)?[0-9]+")
    }

    pub fn float(&mut self) -> Option<String> {
        self.match_(r"(\+|\-)?([0-9]+\.[0-9]*|[0-9]*\.[0-9]+)([eE][-+]?[0-9]+)?")
    }

    pub fn word(&mut self) -> Option<String> {
        self.match_(r"[a-zA-Z_\u00a0-\ufffd][0-9a-zA-Z_\u00a0-\ufffd]*")
    }

    pub fn name(&mut self) -> Option<String> {
        let oldpos = self.pos;
        let rv = self.word();
        if let Some(rv) = rv {
            if self.keywords.contains(rv.as_str()) {
                self.pos = oldpos;
                return None;
            }
            Some(rv)
        } else {
            None
        }
    }

    pub fn dotted_name(&mut self) -> Result<Option<String>> {
        let mut rv = match self.name() {
            Some(name) => name,
            _ => return Ok(None),
        };

        while self.match_(r"\.").is_some() {
            if self.name().is_none() {
                let err: Result<()> = self.error("expecting name");
                return Err(err.err().unwrap());
            }
            let n = self.name().unwrap();
            rv = format!("{}.{}", rv, n);
        }

        Ok(Some(rv))
    }

    pub fn simple_expression(&mut self) -> Result<Option<String>> {
        self.skip_whitespace();
        if self.eol() {
            return Ok(None);
        }

        let start = self.pos;

        while !self.eol() {
            self.skip_whitespace();
            if self.eol() {
                break;
            }
            if self.match_(r"\.").is_some() {
                if self.name().is_none() {
                    let err: Result<()> = self.error("expecting name after dot");
                    return Err(err.err().unwrap());
                }
                continue;
            }
            break;
        }

        Ok(Some(self.text[start..self.pos].to_string()))
    }

    pub fn require(&mut self, thing: &str) -> Result<String> {
        if let Some(rv) = self.match_(thing) {
            Ok(rv)
        } else {
            let err: Result<()> = self.error(&format!("expected '{}' not found", thing));
            return Err(err.err().unwrap());
        }
    }

    pub fn rest(&mut self) -> String {
        self.skip_whitespace();
        let start = self.pos;
        self.pos = self.text.len();
        self.text[start..].to_string()
    }

    pub fn get_location(&self) -> usize {
        return self.line_number;
    }

    /// Returns an opaque representation of the lexer state. This can be
    /// passed to revert to back the lexer up.
    pub fn checkpoint(&self) -> LexerState {
        LexerState {
            filename: self.filename.clone(),
            line_number: self.line_number,
            text: self.text.clone(),
            subblock: self.subblock.clone(),
            pos: self.pos,
        }
    }

    /// Reverts the lexer to the given state. State must have been returned
    /// by a previous checkpoint operation on this lexer.
    pub fn revert(&mut self, state: LexerState) {
        self.filename = state.filename;
        self.line_number = state.line_number;
        self.text = state.text;
        self.subblock = state.subblock;
        self.pos = state.pos;
    }
}
