use crate::lexer::Lexer;
use anyhow::{anyhow, Result};
use std::collections::HashSet;
use std::error;
use std::fmt;
use std::io::empty;

#[derive(Debug)]
pub struct ParseError {
    pub filename: String,
    pub line_number: usize,
    pub message: String,
    pub line: Option<String>,
    pub pos: Option<usize>,
}

impl error::Error for ParseError {}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl ParseError {
    pub fn new(
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

    pub fn to_string(&self) -> String {
        let message = format!(
            "On line {} of {}: {}",
            self.line_number, self.filename, self.message
        );

        // if let Some(line) = &self.line {
        //     message.push_str(&format!("\nline:\n{}", line));
        // }

        // if let Some(pos) = self.pos {
        //     message.push_str(&format!("\n{:>pos$}", "^", pos = pos + 1));
        // }

        message
    }
}

#[derive(Clone, Debug)]
pub enum AST {
    Return(usize, Option<String>),
    Jump(usize, String, bool),
    Scene(usize, Option<String>, String),
    Show(usize, String),
    Hide(usize, String),
    Label(usize, String, Vec<AST>, Option<String>),
    Init(usize, Vec<AST>, i32),
    Say(usize, Option<String>, String, Option<String>),
    Play(usize, String, String),
    UserStatement(usize, String),
    Error,
}

fn parse_image_name(lexer: &mut Lexer) -> Result<Vec<String>> {
    let name = lexer.name().unwrap_or_default();

    let mut rv: Vec<String> = vec![name.clone()];

    loop {
        let name = lexer.name();
        if let Some(n) = name {
            rv.push(n.trim().to_string());
        } else {
            break;
        }
    }
    Ok(rv)
}

pub fn parse_simple_expression_list(input: &str) -> Result<Vec<String>> {
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

pub fn parse_with(_: &mut Lexer, node: AST) -> Result<Vec<AST>> {
    return Ok(vec![node]);
}

pub fn parse_image_specifier(lexer: &mut Lexer) -> Result<(String, Option<String>, String)> {
    let layer: Option<String> = None;
    let image_name: String;
    let expression: Option<String>;

    let image_names = parse_image_name(lexer)?;

    image_name = image_names.join(" ");
    expression = None;

    let layer = layer.unwrap_or_else(|| "master".to_string());

    Ok((image_name, expression, layer))
}

pub fn parse_play_specifier(lexer: &mut Lexer) -> Result<(String)> {
    let play_type = lexer.name().unwrap_or_default();

    if play_type == "music" || play_type == "sound" {
        return Ok(play_type);
    }

    return Err(anyhow!("Play or sound is required"));
}

pub fn parse_audio_filename(lexer: &mut Lexer) -> Result<(String)> {
    let audio_filename = lexer.audio_filename();

    if audio_filename.is_none() {
        return Err(anyhow!("provide mp3, ogg or wav file"));
    }

    return Ok(audio_filename.unwrap());
}

#[derive(Debug)]
pub struct ParameterInfo {
    pub parameters: Vec<(String, Option<String>)>,
    pub positional: Vec<String>,
    pub extrapos: Option<String>,
    pub extrakw: Option<String>,
}

pub fn parse_parameters(l: &mut Lexer) -> Result<Option<ParameterInfo>> {
    let mut parameters = Vec::new();
    let mut positional = Vec::new();
    let mut extrapos = None;
    let mut extrakw = None;
    let mut add_positional = true;
    let mut names = HashSet::new();

    if !l.match_("(").is_some() {
        return Ok(None);
    }

    loop {
        if l.match_(")").is_some() {
            break;
        }

        if l.match_("**").is_some() {
            if extrakw.is_some() {
                l.error("a label may have only one ** parameter")?;
            }

            let name = l.name().unwrap_or_default();
            let name = l.require(&name)?;

            if names.contains(&name) {
                l.error(&format!("parameter {} appears twice.", name))?;
            }

            names.insert(name.clone());
            extrakw = Some(name);
        } else if l.match_("*").is_some() {
            if !add_positional {
                l.error("a label may have only one * parameter")?;
            }

            add_positional = false;

            if let Some(name) = l.name() {
                if names.contains(&name) {
                    l.error(&format!("parameter {} appears twice.", name))?;
                }
                names.insert(name.clone());
                extrapos = Some(name);
            }
        } else {
            let name = l.name().unwrap_or_default();
            let name = l.require(&name)?;

            if names.contains(&name) {
                l.error(&format!("parameter {} appears twice.", name))?;
            }

            names.insert(name.clone());

            let default = None;

            parameters.push((name.clone(), default));

            if add_positional {
                positional.push(name);
            }
        }

        if l.match_(")").is_some() {
            break;
        }

        if !l.match_(",").is_some() {
            l.error("Expected ',' or ')'")?;
        }
    }

    Ok(Some(ParameterInfo {
        parameters,
        positional,
        extrapos,
        extrakw,
    }))
}

pub fn parse_statement(l: &mut Lexer) -> Result<AST> {
    let loc = l.get_location();

    if l.keyword("return").is_some() {
        let nonblock = l.expect_noblock("return statement");
        if nonblock.is_err() {
            return Err(nonblock.err().unwrap());
        }

        let rest = l.rest();

        let eol = l.expect_eol();
        if eol.is_err() {
            return Err(eol.err().unwrap());
        }

        l.advance();
        return Ok(AST::Return(loc, Some(rest)));
    }

    if l.keyword("jump").is_some() {
        let nonblock = l.expect_noblock("jump statement");
        if nonblock.is_err() {
            return Err(nonblock.err().unwrap());
        }

        let target = l.name().unwrap_or_default();

        l.expect_eol()?;
        l.advance();
        return Ok(AST::Jump(loc, target, false));
    }

    if l.keyword("scene").is_some() {
        l.expect_noblock("scene statement")?;

        let layer = "master".to_string();
        if l.eol() {
            l.advance();
            return Ok(AST::Scene(loc, None, layer));
        }

        let imspec = parse_image_specifier(l)?.0;

        l.advance();
        return Ok(AST::Scene(loc, Some(imspec), layer));
    }

    if l.keyword("show").is_some() {
        let imspec = parse_image_specifier(l)?.0;
        let rv = parse_with(l, AST::Show(loc, imspec))?[0].clone();

        l.expect_eol()?;
        l.expect_noblock("show statement")?;
        l.advance();
        return Ok(rv);
    }

    if l.keyword("hide").is_some() {
        let imspec = parse_image_specifier(l)?.0;
        let rv = parse_with(l, AST::Hide(loc, imspec))?[0].clone();

        l.expect_eol()?;
        l.expect_noblock("hide statement")?;
        l.advance();
        return Ok(rv);
    }

    if l.keyword("play").is_some() {
        let play_type = parse_play_specifier(l)?;

        let filename = parse_audio_filename(l)?;

        l.expect_eol()?;
        l.advance();

        return Ok(AST::Play(loc, play_type, filename));
    }

    if l.keyword("label").is_some() {
        let name = l.name().unwrap_or_default();

        let (block_ast, block_err) = parse_block(&mut l.subblock_lexer(false));

        if block_err.len() > 0 {
            for err in block_err {
                l.error(&err)?;
            }
        }

        l.advance();

        // let label = AST::Label(loc, name, block_ast, parameters);
        let label = AST::Label(loc, name, block_ast, None);
        return Ok(label);
    }

    if l.keyword("init").is_some() {
        let priority = l.integer().map_or(0, |p| p.parse::<i32>().unwrap_or(0));

        let (block_ast, block_err) = {
            l.require(":")?;
            l.expect_eol()?;
            l.expect_block("init statement")?;
            parse_block(&mut l.subblock_lexer(false))
        };

        if block_err.len() > 0 {
            for err in block_err {
                l.error(&err)?;
            }
        }

        l.advance();
        let ast = AST::Init(loc, block_ast, priority);
        return Ok(ast);
    }

    // Handle user statements or say statements.
    let state = l.checkpoint();

    if let Some(word) = l.word() {
        let text = l.text();
        l.expect_noblock(&format!("{} statement", word))?;
        l.advance();

        let rv = AST::UserStatement(loc, text.clone());

        return Ok(rv);
    }

    l.revert(state.clone());
    let what = l.string();

    if let Some(what) = what {
        if l.eol() {
            l.expect_noblock("say statement")?;
            l.advance();
            return Ok(AST::Say(loc, None, what, None));
        }
    }

    l.revert(state.clone());

    let who = l.simple_expression()?;
    let what = l.string();

    if let (Some(who), Some(what)) = (who, what) {
        l.expect_eol()?;
        l.expect_noblock("say statement")?;
        l.advance();
        return Ok(AST::Say(loc, Some(who), what, None));
    }

    let err = l.error("expected statement.").err().unwrap();
    return Err(err);
}

pub fn parse_block(l: &mut Lexer) -> (Vec<AST>, Vec<String>) {
    let mut rv = Vec::new();
    let mut parse_errors = Vec::new();

    l.advance();

    while !l.eob() {
        match parse_statement(l) {
            Ok(stmt) => rv.push(stmt),
            Err(e) => {
                parse_errors.push(format!("{}", e));
                l.advance();
            }
        }
    }

    (rv, parse_errors)
}
