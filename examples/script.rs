use renpy_parser::{group_logical_lines, list_logical_lines};

fn main() {
    let lines = list_logical_lines("assets/script.rpy");
    if lines.is_err() {
        println!("error, {:?}", lines.err());
        return;
    }

    let lines = lines.unwrap();

    for line in &lines {
        println!("{:?}", line);
    }

    let nested = group_logical_lines(lines);
    if nested.is_err() {
        println!("error, {:?}", nested.err());
        return;
    }

    println!("----");

    let blocks = nested.unwrap();

    for block in blocks {
        println!("{:?}", block);
    }

    println!("all ok!");
}
