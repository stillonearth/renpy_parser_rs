use renpy_parser::parse_scenario_from_file;

fn main() {
    let (ast, _parse_error) = parse_scenario_from_file("assets/script.rpy").unwrap();

    for ast in ast {
        println!("ast: {:?}", ast);
    }
}
