pub mod lexer;
pub mod parsers;

use anyhow::Result;
use lexer::{Block, Lexer};
use parsers::{parse_block, ParseError, AST};
use regex::Regex;
use std::{fs::File, io::Read, path::Path};

#[derive(Debug, Clone)]
pub struct LogicalLine {
    filename: String,
    line_number: usize,
    text: String,
}

pub fn parse_logical_lines(mut data: String, filename: String) -> Result<Vec<LogicalLine>> {
    // Replace Windows line endings
    data = data.replace("\r\n", "\n");

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
                Some(line),
                Some(pos),
            );

            return Err(parse_error.into());
        }
    }

    Ok(rv)
}

/// Reads the specified filename and divides it into logical lines
pub fn list_logical_lines(filename: &str) -> Result<Vec<LogicalLine>> {
    let mut file = File::open(Path::new(filename))?;
    let mut data = String::new();
    file.read_to_string(&mut data)?;

    parse_logical_lines(data, filename.to_string())
}

/// Groups logical lines into blocks based on indentation
pub fn group_logical_lines(lines: Vec<LogicalLine>) -> Result<Vec<Block>> {
    fn depth_split(line: &str) -> (usize, String) {
        let mut depth = 0;
        let chars = line.chars();
        let mut chars_copy = chars.clone();

        for c in chars {
            if c == ' ' {
                depth += 1;
            } else {
                break;
            }
        }

        for _ in 0..depth {
            chars_copy.next();
        }

        (depth, chars_copy.collect())
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

            let (subblocks, new_i) = gll_core(lines, i, line_depth + 1)?;
            i = new_i;

            rv.push(Block {
                filename: line.filename.clone(),
                line_number: line.line_number,
                text: rest,
                subblocks,
            });
        }

        Ok((rv, i))
    }

    let (blocks, _) = gll_core(&lines, 0, 0)?;
    Ok(blocks)
}

pub fn parse_scenario(filename: &str) -> Result<(Vec<AST>, Vec<String>)> {
    let lines = list_logical_lines(filename).unwrap();
    let blocks = group_logical_lines(lines).unwrap();
    let l = &mut Lexer::new(blocks.clone(), true);

    Ok(parse_block(l))
}
