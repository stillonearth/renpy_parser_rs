use renpy_parser::{parse_scenario_from_file, parsers::ASTVec};

fn main() {
    let (ast, _parse_error) = parse_scenario_from_file("assets/script.rpy").unwrap();

    println!("{}", ASTVec(&ast));
}
