use crate::lexer::Lexer;
use anyhow::{anyhow, Result};
use std::error;
use std::fmt;
use std::fmt::format;

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
    Define(usize, String),
    Hide(usize, String),
    Jump(usize, String, bool),
    Label(usize, String, Vec<AST>, Option<String>),
    Play(usize, String, String),
    Return(usize, Option<String>),
    Say(usize, Option<String>, String),
    Scene(usize, Option<String>, String),
    Show(usize, String),
    Stop(usize, String, Option<String>, Option<f32>),
    GameMechanic(usize, String),
    LLMGenerate(usize, String, Option<String>),
    Comment(usize, String),
    Error,
}

impl AST {
    pub fn index(&self) -> usize {
        *match self {
            AST::Define(i, _) => i,
            AST::Hide(i, _) => i,
            AST::Jump(i, _, _) => i,
            AST::Label(i, _, _, _) => i,
            AST::Play(i, _, _) => i,
            AST::Return(i, _) => i,
            AST::Say(i, _, _) => i,
            AST::Scene(i, _, _) => i,
            AST::Show(i, _) => i,
            AST::Stop(i, _, _, _) => i,
            AST::GameMechanic(i, _) => i,
            AST::LLMGenerate(i, _, _) => i,
            AST::Comment(i, _) => i,
            AST::Error => todo!(),
        }
    }

    pub fn set_index(&mut self, index: usize) {
        *match self {
            AST::Define(i, _) => i,
            AST::Hide(i, _) => i,
            AST::Jump(i, _, _) => i,
            AST::Label(i, _, _, _) => i,
            AST::Play(i, _, _) => i,
            AST::Return(i, _) => i,
            AST::Say(i, _, _) => i,
            AST::Scene(i, _, _) => i,
            AST::Show(i, _) => i,
            AST::Stop(i, _, _, _) => i,
            AST::GameMechanic(i, _) => i,
            AST::LLMGenerate(i, _, _) => i,
            AST::Comment(i, _) => i,
            AST::Error => todo!(),
        } = index;
    }
}

pub struct ASTVec<'a>(pub &'a [AST]);

impl<'a> fmt::Display for ASTVec<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // let result = ast_list_to_string(self);

        let mut strings: Vec<String> = Vec::new();
        for element in self.0.iter() {
            let str = format!("{}", *element);
            strings.push(str);
        }

        write!(f, "{}", strings.join("\n"))
    }
}

impl fmt::Display for AST {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let result = match self {
            AST::Define(i, definition) => write!(f, "define {}", *definition),
            AST::Hide(i, image) => write!(f, "hide {}", *image),
            AST::Jump(i, label, _) => write!(f, "jump {}", *label),
            AST::Label(i, name, block, _) => {
                // let block_string = ast_list_to_string(block);
                let block_string = format!("{}", ASTVec(&block));
                let block_string = block_string.replace("\n", "\n    ");
                write!(f, "label {}:\n    {}", *name, block_string)
            }
            AST::Play(i, sound_type, filename) => {
                write!(f, "play {} \"{}\"", *sound_type, *filename)
            }
            AST::Return(i, _) => write!(f, "return"),
            AST::Say(i, who, what) => {
                if let Some(who) = who.clone() {
                    write!(f, "{} \"{}\"", who, *what)
                } else {
                    write!(f, "\"{}\"", *what)
                }
            }
            AST::Scene(i, name, _layer) => {
                let name = name.clone().unwrap_or_default();
                write!(f, "scene {}", name)
            }
            AST::Show(i, image_name) => write!(f, "show {}", *image_name),
            AST::Stop(loc, audio_specifier, effect, length) => {
                write!(f, "stop {}", *audio_specifier)
            }
            AST::GameMechanic(i, mechanic) => write!(f, "game_mechanic \"{}\"", *mechanic),
            AST::LLMGenerate(i, character, prompt) => {
                let prompt = if prompt.is_some() {
                    prompt.clone().unwrap()
                } else {
                    "".to_string()
                };
                write!(f, "llm_generate {} \"{}\"", *character, prompt)
            }
            AST::Comment(i, comment) => write!(f, "#{}", *comment),
            AST::Error => write!(f, ""),
        };

        return result;
    }
}

pub fn inject_node(ast: Vec<AST>, node: AST) -> Vec<AST> {
    let node_index = node.index();

    // Step 1: Shift all nodes with index >= node_index
    let mut shifted_ast: Vec<AST> = ast
        .into_iter()
        .map(|mut item| {
            if item.index() >= node_index {
                item.set_index(item.index() + 1);
            }
            item
        })
        .collect();

    // Step 2: Insert the new node at the correct position
    shifted_ast.push(node);

    // Sort the vector by index to maintain order
    shifted_ast.sort_by_key(|item| item.index());

    shifted_ast
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
    Ok(vec![node])
}

pub fn parse_image_specifier(lexer: &mut Lexer) -> Result<(String, Option<String>, String)> {
    let layer: Option<String> = None;

    let image_names = parse_image_name(lexer)?;

    let image_name: String = image_names.join(" ");
    let expression: Option<String> = None;

    let layer = layer.unwrap_or_else(|| "master".to_string());

    Ok((image_name, expression, layer))
}

pub fn parse_audio_specifier(lexer: &mut Lexer) -> Result<String> {
    let play_type = lexer.name().unwrap_or_default();

    if play_type == "music" || play_type == "sound" {
        return Ok(play_type);
    }

    Err(anyhow!("Play or sound is required"))
}

pub fn parse_audio_filename(lexer: &mut Lexer) -> Result<String> {
    let audio_filename = lexer.audio_filename();

    if audio_filename.is_none() {
        return Err(anyhow!("provide mp3, ogg or wav file"));
    }

    Ok(audio_filename.unwrap().replace("\"", ""))
}

#[derive(Debug)]
pub struct ParameterInfo {
    pub parameters: Vec<(String, Option<String>)>,
    pub positional: Vec<String>,
    pub extrapos: Option<String>,
    pub extrakw: Option<String>,
}

pub fn parse_statement(l: &mut Lexer) -> Result<AST> {
    let loc = l.get_location();

    if l.keyword("^#").is_some() {
        let rest = l.rest();

        let eol = l.expect_eol();
        if eol.is_err() {
            return Err(eol.err().unwrap());
        }

        l.advance();
        return Ok(AST::Comment(loc, rest));
    }

    if l.keyword("^return").is_some() {
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

    if l.keyword("^jump").is_some() {
        let nonblock = l.expect_noblock("jump statement");
        if nonblock.is_err() {
            return Err(nonblock.err().unwrap());
        }

        let target = l.name().unwrap_or_default();

        l.expect_eol()?;
        l.advance();
        return Ok(AST::Jump(loc, target, false));
    }

    if l.keyword("^scene").is_some() {
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

    if l.keyword("^game_mechanic").is_some() {
        let argument = l.string();

        if argument.is_none() {
            l.error("Expected a string after 'game_mechanic' keyword.")?;
        }

        l.expect_eol()?;
        l.expect_noblock("game_mechanic statement")?;
        l.advance();

        return Ok(AST::GameMechanic(loc, argument.unwrap()));
    }

    if l.keyword("^llm_generate").is_some() {
        if let Some(who) = l.word() {
            let prompt = l.string();

            l.expect_eol()?;
            l.expect_noblock("game_mechanic statement")?;
            l.advance();

            return Ok(AST::LLMGenerate(loc, who, prompt));
        }

        l.error("Expected word after 'llm_generate' keyword.")?;
    }

    if l.keyword("^show").is_some() {
        let imspec = parse_image_specifier(l)?.0;
        let rv = parse_with(l, AST::Show(loc, imspec))?[0].clone();

        l.expect_eol()?;
        l.expect_noblock("show statement")?;
        l.advance();
        return Ok(rv);
    }

    if l.keyword("^hide").is_some() {
        let imspec = parse_image_specifier(l)?.0;
        let rv = parse_with(l, AST::Hide(loc, imspec))?[0].clone();

        l.expect_eol()?;
        l.expect_noblock("hide statement")?;
        l.advance();
        return Ok(rv);
    }

    if l.keyword("^play").is_some() {
        let play_type = parse_audio_specifier(l)?;

        let filename = parse_audio_filename(l)?;

        l.expect_eol()?;
        l.advance();

        return Ok(AST::Play(loc, play_type, filename));
    }

    if l.keyword("^stop").is_some() {
        let audio_specifier = parse_audio_specifier(l)?;

        let (effect, length) = l.stop_arguments();

        l.expect_eol()?;
        l.advance();

        return Ok(AST::Stop(loc, audio_specifier, effect, length));
    }

    if l.keyword("^label").is_some() {
        let name = l.name().unwrap_or_default();

        let (block_ast, block_err) = parse_block(&mut l.subblock_lexer(false));

        if !block_err.is_empty() {
            for err in block_err {
                l.error(&err)?;
            }
        }

        l.advance();

        let label = AST::Label(loc, name, block_ast, None);
        return Ok(label);
    }

    if l.keyword("^define").is_some() {
        let definition = l.rest();
        l.expect_eol()?;
        l.advance();

        let label = AST::Define(loc, definition);
        return Ok(label);
    }

    // Handle user statements or say statements.
    let state = l.checkpoint();

    if let Some(word) = l.word() {
        let text = l.string();
        if text.is_none() {
            l.error("empty text in say statement")?;
        }

        l.expect_noblock(&format!("{} statement", word))?;
        l.advance();

        let rv = AST::Say(loc, Some(word), text.unwrap());
        return Ok(rv);
    }

    l.revert(state.clone());
    let what = l.string();

    if let Some(what) = what {
        if l.eol() {
            l.expect_noblock("say statement")?;
            l.advance();

            return Ok(AST::Say(loc, None, what));
        }
    }

    let err = l.error("expected statement.").err().unwrap();
    Err(err)
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
