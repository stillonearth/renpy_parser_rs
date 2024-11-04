pub mod lexer;

use anyhow::{anyhow, Result};
use regex::Regex;
use std::env;
use std::error;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::lexer::Lexer;

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

fn parse_image_name(lexer: &mut Lexer) -> Vec<String> {
    let name = lexer.name().unwrap_or_default();
    let mut rv: Vec<String> = vec![lexer.require(&name)];

    loop {
        if let Some(n) = lexer.simple_expression() {
            rv.push(n.trim().to_string());
        } else {
            break;
        }
    }

    rv
}

fn parse_simple_expression_list(input: &str) -> Result<Vec<String>> {
    let mut parts = Vec::new();

    let mut iter = input.split(',');
    if let Some(first) = iter.next() {
        parts.push(first.trim().to_string());

        for part in iter {
            parts.push(part.trim().to_string());
        }
    } else {
        return Err(anyhow!("At least one simple expression is required"));
    }

    Ok(parts)
}

enum AST {
    Return(usize, Option<String>),
    Jump(usize, String, bool),
    Scene(usize, Option<String>, String),
    Show(usize, String),
    Hide(usize, String),
    Label(usize, String, Vec<AST>, Option<String>),
    Init(usize, Vec<AST>, i32),
    Say(usize, Option<String>, String, Option<String>),
    UserStatement(usize, String),
}

// fn parse_statement(l: &mut Lexer) -> AST {
//     let loc = l.get_location();

//     if l.keyword("return").is_some() {
//         l.expect_noblock("return statement");
//         let rest = l.rest();
//         l.expect_eol();
//         l.advance();
//         return AST::Return(loc, Some(rest));
//     }

//     if l.keyword("jump").is_some() {
//         l.expect_noblock("jump statement");

//         let expression = if l.keyword("expression").is_some() {
//             true
//         } else {
//             false
//         };

//         let target = if expression {
//             l.require(&l.simple_expression().unwrap_or_default())
//         } else {
//             l.require(&l.name().unwrap_or_default())
//         };

//         l.expect_eol();
//         l.advance();
//         return AST::Jump(loc, target, expression);
//     }

//     if l.keyword("scene").is_some() {
//         l.expect_noblock("scene statement");

//         let layer = if l.keyword("onlayer").is_some() {
//             l.require(&l.name().unwrap_or_default())
//         } else {
//             "master".to_string()
//         };

//         if l.eol() {
//             l.advance();
//             return AST::Scene(loc, None, layer);
//         }

//         let imspec = parse_image_specifier(l);
//         let rv = parse_with(l, AST::Scene(loc, imspec.clone(), imspec[4].clone()));

//         l.expect_eol();
//         l.advance();
//         return rv;
//     }

//     if l.keyword("show") {
//         let imspec = parse_image_specifier(l);
//         let rv = parse_with(l, AST::Show(loc, imspec));

//         l.expect_eol();
//         l.expect_noblock("show statement");
//         l.advance();
//         return rv;
//     }

//     if l.keyword("hide") {
//         let imspec = parse_image_specifier(l);
//         let rv = parse_with(l, AST::Hide(loc, imspec));

//         l.expect_eol();
//         l.expect_noblock("hide statement");
//         l.advance();
//         return rv;
//     }

//     if l.keyword("label") {
//         let name = l.require(Lexer::name);
//         let parameters = parse_parameters(l);

//         l.require(":");
//         l.expect_eol();

//         let block = parse_block(&mut l.subblock_lexer());

//         l.advance();
//         return AST::Label(loc, name, block, parameters);
//     }

//     if l.keyword("init") {
//         let priority = l.integer().map_or(0, |p| p.parse::<i32>().unwrap_or(0));

//         let block = if l.keyword("python") {
//             let hide = l.keyword("hide");
//             l.require(":");
//             l.expect_block("python block");
//             vec![AST::Python(loc, l.python_block(), hide)]
//         } else {
//             l.require(":");
//             l.expect_eol();
//             l.expect_block("init statement");
//             parse_block(&mut l.subblock_lexer())
//         };

//         l.advance();
//         return AST::Init(loc, block, priority);
//     }

//     // Handle user statements or say statements.
//     let state = l.checkpoint();

//     if let Some(word) = l.word() {
//         let text = l.text;
//         l.expect_noblock(&format!("{} statement", word));
//         l.advance();

//         renpy::exports::push_error_handler(l.error);
//         let rv = AST::UserStatement(loc, text.clone());
//         renpy::exports::pop_error_handler();

//         return rv;
//     }

//     l.revert(state);
//     let what = l.string();

//     if let Some(what) = what {
//         if l.eol() {
//             l.expect_noblock("say statement");
//             l.advance();
//             return AST::Say(loc, None, what, None);
//         }
//     }

//     l.revert(state);

//     let who = l.simple_expression();
//     let what = l.string();

//     if let (Some(who), Some(what)) = (who, what) {
//         l.expect_eol();
//         l.expect_noblock("say statement");
//         l.advance();
//         return AST::Say(loc, Some(who), what, None);
//     }

//     l.error("expected statement.");
//     // Placeholder return in case of error
//     AST::Error
// }

// fn parse_block(l: &mut Lexer) -> Vec<AST> {
//     let mut rv = Vec::new();

//     l.advance();

//     while !l.eob() {
//         match parse_statement(l) {
//             Ok(stmt) => rv.push(stmt),
//             Err(e) => {
//                 parse_errors.push(e.message);
//                 l.advance();
//             }
//         }
//     }

//     rv
// }

fn parse_image_specifier(
    lexer: &mut Lexer,
) -> (
    String,
    Option<String>,
    Option<String>,
    Vec<String>,
    String,
    Option<String>,
    Vec<String>,
) {
    let mut tag: Option<String> = None;
    let mut layer: Option<String> = None;
    let mut at_list: Vec<String> = Vec::new();
    let mut zorder: Option<String> = None;
    let mut behind: Vec<String> = Vec::new();
    let image_name: String;
    let expression: Option<String>;

    if lexer.keyword("expression").is_some() || lexer.keyword("image").is_some() {
        let exp = lexer.simple_expression().unwrap_or_default();
        expression = Some(lexer.require(&exp).trim().to_string());
        image_name = expression.as_ref().unwrap().clone();
    } else {
        let image_names = parse_image_name(lexer);
        image_name = image_names[0].clone();
        expression = None;
    }

    let layer = layer.unwrap_or_else(|| "master".to_string());

    (image_name, expression, tag, at_list, layer, zorder, behind)
}
