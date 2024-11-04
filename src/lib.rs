use anyhow::{Error, Result};
use regex::Regex;
use std::collections::HashSet;
use std::env;
use std::error;
use std::fmt;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

#[derive(Debug)]
pub struct ParseError {
    filename: String,
    line_number: usize,
    message: String,
    line: Option<String>,
    pos: Option<usize>,
}

impl error::Error for ParseError {}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl ParseError {
    fn new(
        filename: String,
        line_number: usize,
        message: String,
        line: Option<String>,
        pos: Option<usize>,
    ) -> Self {
        ParseError {
            filename,
            line_number,
            message,
            line,
            pos,
        }
    }

    fn to_string(&self) -> String {
        let mut message = format!(
            "On line {} of {}: {}",
            self.line_number, self.filename, self.message
        );

        if let Some(line) = &self.line {
            message.push_str(&format!("\n{}", line));
        }

        if let Some(pos) = self.pos {
            message.push_str(&format!("\n{:>pos$}", "^", pos = pos + 1));
        }

        message
    }
}

#[derive(Debug, Clone)]
struct LogicalLine {
    filename: String,
    line_number: usize,
    text: String,
}

#[derive(Debug)]
struct Lexer {
    block: Vec<LogicalLine>,
    current_line: Option<LogicalLine>,
    line_index: usize,
    pos: usize,
    init: bool,
    keywords: HashSet<String>,
}

impl Lexer {
    fn new(block: Vec<LogicalLine>, init: bool) -> Self {
        let mut keywords = HashSet::new();
        keywords.insert("as".to_string());
        keywords.insert("at".to_string());
        keywords.insert("behind".to_string());
        keywords.insert("call".to_string());
        keywords.insert("expression".to_string());
        keywords.insert("hide".to_string());
        keywords.insert("if".to_string());
        keywords.insert("image".to_string());
        keywords.insert("init".to_string());
        keywords.insert("jump".to_string());
        keywords.insert("menu".to_string());
        keywords.insert("onlayer".to_string());
        keywords.insert("python".to_string());
        keywords.insert("return".to_string());
        keywords.insert("scene".to_string());
        keywords.insert("set".to_string());
        keywords.insert("show".to_string());
        keywords.insert("with".to_string());
        keywords.insert("while".to_string());
        keywords.insert("zorder".to_string());

        Lexer {
            block,
            current_line: None,
            line_index: 0,
            pos: 0,
            init,
            keywords,
        }
    }

    fn advance(&mut self) -> bool {
        self.line_index += 1;
        if self.line_index >= self.block.len() {
            self.current_line = None;
            false
        } else {
            self.current_line = Some(self.block[self.line_index].clone());
            self.pos = 0;
            true
        }
    }

    fn match_regexp(&mut self, pattern: &str) -> Option<String> {
        if let Some(line) = &self.current_line {
            let regex = regex::Regex::new(pattern).unwrap();
            if let Some(capture) = regex.find(&line.text[self.pos..]) {
                if capture.start() == 0 {
                    self.pos += capture.end();
                    return Some(capture.as_str().to_string());
                }
            }
        }
        None
    }

    fn skip_whitespace(&mut self) {
        self.match_regexp(r"^\s+");
    }

    fn keyword(&mut self, word: &str) -> bool {
        self.skip_whitespace();
        if let Some(matched) = self.match_regexp(&format!(r"^{}\b", word)) {
            matched == word
        } else {
            false
        }
    }

    fn name(&mut self) -> Option<String> {
        self.skip_whitespace();
        if let Some(word) = self.match_regexp(r"^[a-zA-Z_][a-zA-Z0-9_]*") {
            if !self.keywords.contains(&word) {
                return Some(word);
            }
        }
        None
    }

    fn string(&mut self) -> Option<String> {
        self.skip_whitespace();
        let patterns = vec![
            r#"^r?"([^\\"]|\\.)*""#,
            r#"^r?'([^\\']|\\.)*'"#,
            r#"^r?`([^\\`]|\\.)*`"#,
        ];

        for pattern in patterns {
            if let Some(mut s) = self.match_regexp(pattern) {
                let raw = s.starts_with('r');
                if raw {
                    s = s[1..].to_string();
                }
                // Strip delimiters
                s = s[1..s.len() - 1].to_string();

                if !raw {
                    // Handle escape sequences and whitespace
                    s = s.replace("\\n", "\n");
                    let re = regex::Regex::new(r"\s+").unwrap();
                    s = re.replace_all(&s, " ").to_string();
                }
                return Some(s);
            }
        }
        None
    }

    fn error(&self, msg: &str) -> ParseError {
        if let Some(line) = &self.current_line {
            ParseError::new(
                line.filename.clone(),
                line.line_number,
                msg.to_string(),
                Some(line.text.clone()),
                Some(self.pos),
            )
        } else {
            ParseError::new("unknown".to_string(), 0, msg.to_string(), None, None)
        }
    }
}

#[derive(Debug)]
enum Statement {
    Say {
        who: Option<String>,
        what: String,
        with_: Option<String>,
    },
    Jump {
        target: String,
        expression: bool,
    },
    Menu {
        items: Vec<(String, String, Vec<Statement>)>,
        set: Option<String>,
        with_: Option<String>,
    },
    Python {
        code: String,
        hide: bool,
    },
    If {
        entries: Vec<(String, Vec<Statement>)>,
    },
    While {
        condition: String,
        block: Vec<Statement>,
    },
    Label {
        name: String,
        block: Vec<Statement>,
        parameters: Option<ParameterInfo>,
    },
    Pass,
}

#[derive(Debug)]
struct ParameterInfo {
    parameters: Vec<(String, Option<String>)>,
    positional: Vec<String>,
    extrapos: Option<String>,
    extrakw: Option<String>,
}

/// Reads the specified filename and divides it into logical lines
pub fn list_logical_lines(filename: &str) -> Result<Vec<LogicalLine>> {
    let mut file = File::open(Path::new(filename))?;
    let mut data = String::new();
    file.read_to_string(&mut data)?;

    // Replace Windows line endings
    data = data.replace("\r\n", "\n");

    // Handle path elision if environment variable is set
    let filename = if let Ok(path_elide) = env::var("RENPY_PATH_ELIDE") {
        let parts: Vec<&str> = path_elide.split(':').collect();
        if parts.len() == 2 {
            filename.replace(parts[0], parts[1])
        } else {
            filename.to_string()
        }
    } else {
        filename.to_string()
    };

    // Add newlines to fix lousy editors
    data.push_str("\n\n");

    let mut rv = Vec::new();
    let mut number = 1;
    let mut pos = 0;
    let chars: Vec<char> = data.chars().collect();

    // Skip BOM if present
    if chars.first() == Some(&'\u{feff}') {
        pos += 1;
    }

    while pos < chars.len() {
        let start_number = number;
        let mut line = String::new();
        let mut parendepth = 0;

        while pos < chars.len() {
            let c = chars[pos];

            if c == '\t' {
                let parse_error = ParseError::new(
                    filename,
                    number,
                    "Tab characters are not allowed in Ren'Py scripts".to_string(),
                    Some(line),
                    Some(pos),
                );
                return Err(parse_error.into());
            }

            if c == '\n' {
                number += 1;
            }

            if c == '\n' && parendepth == 0 {
                // Check if line is not blank
                let re = Regex::new(r"^\s*$").unwrap();
                if !re.is_match(&line) {
                    rv.push(LogicalLine {
                        filename: filename.clone(),
                        line_number: start_number,
                        text: line.clone(),
                    });
                }
                pos += 1;
                break;
            }

            // Handle backslash/newline
            if c == '\\' && pos + 1 < chars.len() && chars[pos + 1] == '\n' {
                pos += 2;
                number += 1;
                line.push('\n');
                continue;
            }

            // Handle parentheses
            match c {
                '(' | '[' | '{' => parendepth += 1,
                '}' | ']' | ')' if parendepth > 0 => parendepth -= 1,
                _ => {}
            }

            // Handle comments
            if c == '#' {
                while pos < chars.len() && chars[pos] != '\n' {
                    pos += 1;
                }
                continue;
            }

            // Handle strings
            if c == '"' || c == '\'' || c == '`' {
                let delim = c;
                line.push(c);
                pos += 1;

                let mut escape = false;
                while pos < chars.len() {
                    let c = chars[pos];

                    if c == '\n' {
                        number += 1;
                    }

                    if escape {
                        escape = false;
                        line.push(c);
                        pos += 1;
                        continue;
                    }

                    if c == delim {
                        line.push(c);
                        pos += 1;
                        break;
                    }

                    if c == '\\' {
                        escape = true;
                    }

                    line.push(c);
                    pos += 1;
                }
                continue;
            }

            line.push(c);
            pos += 1;
        }

        if pos >= chars.len() && !line.is_empty() {
            let parse_error = ParseError::new(
                filename.to_string(),
                start_number,
                "is not terminated with a newline (check quotes and parenthesis)".to_string(),
                None,
                Some(pos),
            );

            return Err(parse_error.into());
        }
    }

    Ok(rv)
}

/// Represents a block of logical lines
#[derive(Debug)]
pub struct Block {
    filename: String,
    line_number: usize,
    content: String,
    sub_blocks: Vec<Block>,
}

/// Groups logical lines into blocks based on indentation
pub fn group_logical_lines(lines: Vec<LogicalLine>) -> Result<(Vec<Block>)> {
    fn depth_split(line: &str) -> (usize, String) {
        let mut depth = 0;
        let mut chars = line.chars();

        while let Some(c) = chars.next() {
            if c == ' ' {
                depth += 1;
            } else {
                break;
            }
        }

        (depth, chars.collect())
    }

    fn gll_core(
        lines: &[LogicalLine],
        start_index: usize,
        min_depth: usize,
    ) -> Result<(Vec<Block>, usize)> {
        let mut rv = Vec::new();
        let mut i = start_index;
        let mut depth: Option<usize> = None;

        while i < lines.len() {
            let line = &lines[i];
            let (line_depth, rest) = depth_split(&line.text);

            if line_depth < min_depth {
                break;
            }

            if depth.is_none() {
                depth = Some(line_depth);
            }

            if depth != Some(line_depth) {
                let err = ParseError::new(
                    line.filename.clone(),
                    line.line_number,
                    "indentation mismatch".to_string(),
                    None,
                    None,
                );
                return Err(err.into());
            }

            i += 1;

            let (sub_blocks, new_i) = gll_core(lines, i, line_depth + 1)?;
            i = new_i;

            rv.push(Block {
                filename: line.filename.clone(),
                line_number: line.line_number,
                content: rest,
                sub_blocks,
            });
        }

        Ok((rv, i))
    }

    let (blocks, _) = gll_core(&lines, 0, 0)?;
    Ok(blocks)
}
