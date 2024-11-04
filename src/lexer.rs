use std::collections::HashSet;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct ParseError {
    pub filename: String,
    pub line_number: usize,
    pub message: String,
    pub line_text: String,
    pub position: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}, line {}: {}\n{}\n{}^",
            self.filename,
            self.line_number,
            self.message,
            self.line_text,
            " ".repeat(self.position)
        )
    }
}

impl Error for ParseError {}

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

    fn match_(&mut self, regexp: &str) -> Option<String> {
        self.skip_whitespace();
        self.match_regexp(regexp)
    }

    pub fn keyword(&mut self, regexp: &str) -> Option<String> {
        self.match_(&format!(r"{}\b", regexp))
    }

    fn error(&self, msg: &str) -> ! {
        Err(ParseError {
            filename: self.filename.clone(),
            line_number: self.line_number,
            message: msg.to_string(),
            line_text: self.text.clone(),
            position: self.pos,
        })
        .unwrap()
    }

    pub fn eol(&mut self) -> bool {
        self.skip_whitespace();
        self.pos >= self.text.len()
    }

    pub fn expect_eol(&mut self) {
        if !self.eol() {
            self.error("end of line expected");
        }
    }

    pub fn expect_noblock(&mut self, stmt: &str) {
        if !self.subblock.is_empty() {
            self.error(&format!("{} does not expect a block. Please check the indentation of the line after this one.", stmt));
        }
    }

    pub fn expect_block(&mut self, stmt: &str) {
        if self.subblock.is_empty() {
            self.error(&format!("{} expects a non-empty block.", stmt));
        }
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

    pub fn dotted_name(&mut self) -> Option<String> {
        let mut rv = match self.name() {
            Some(name) => name,
            None => return None,
        };

        while self.match_(r"\.").is_some() {
            let n = match self.name() {
                Some(name) => name,
                None => return self.error("expecting name"),
            };
            rv = format!("{}.{}", rv, n);
        }

        Some(rv)
    }

    pub fn simple_expression(&mut self) -> Option<String> {
        self.skip_whitespace();
        if self.eol() {
            return None;
        }

        let start = self.pos;

        while !self.eol() {
            self.skip_whitespace();
            if self.eol() {
                break;
            }
            if self.match_(r"\.").is_some() {
                if self.name().is_none() {
                    return self.error("expecting name after dot");
                }
                continue;
            }
            break;
        }

        Some(self.text[start..self.pos].to_string())
    }

    pub fn require(&mut self, thing: &str) -> String {
        if let Some(rv) = self.match_(thing) {
            rv
        } else {
            self.error(&format!("expected '{}' not found", thing))
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
}
