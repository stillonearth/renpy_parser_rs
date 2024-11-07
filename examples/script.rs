use renpy_parser::{group_logical_lines, lexer::Lexer, list_logical_lines, parsers::parse_block};

fn main() {
    let lines = list_logical_lines("assets/script.rpy").unwrap();
    let blocks = group_logical_lines(lines).unwrap();

    let l = &mut Lexer::new(blocks.clone(), true);

    println!("blocks len, {}", blocks.clone().len());

    let (asts, _) = parse_block(l);

    for ast in asts {
        println!("ast: {:?}", ast);
    }
}
