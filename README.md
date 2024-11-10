# renpy_parser_rs

<img src="assets/mascot.jpg" width="20%" />

Ren'Py Scenario File Parser in Rust (translated from Ren'Py's parser.py)

This parser handles a subset of the Ren'Py scripting language, excluding support for variables, expressions, and Python code.

Supported keywords:

- "hide"
- "init"
- "jump"
- "return"
- "scene"
- "show"
- "stop"

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
