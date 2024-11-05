# renpy-rs

![image](assets/mascot.jpg)

Renpy scenario file parser in rust. Translated from RenPy's `parser.py`.

This is a subset of renpy language, dropping support for variables, expressions and python code.

Supported keywords:

```"hide", "init", "jump", "return", "scene", "show"```

## Usage

```rust
use renpy_parser::{group_logical_lines, lexer::Lexer, list_logical_lines, parsers::parse_block};

fn main() {
    let lines = list_logical_lines("assets/script.rpy").unwrap();
    let blocks = group_logical_lines(lines).unwrap();
    let l = &mut Lexer::new(blocks.clone(), true);
    let (asts, errors) = parse_block(l);
    for ast in asts {
        println!("ast: {:?}", ast);
    }
}
```
