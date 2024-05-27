use std::io::{self, Write};

const INPUT_MSG: &'static str = "Expected Input: <file to remove>";

fn main() {
    let mut args = std::env::args();
    if args.len() != 2 {
        println!("{}", INPUT_MSG);
        return;
    }
    let filename = args.nth(1).unwrap();
    print!("Delete Contents[Y/n] "); io::stdout().flush().unwrap();
    let mut y_n = String::with_capacity(5);
    io::stdin().read_line(&mut y_n).expect("Error reading input");
    if y_n.trim().is_empty() || y_n.trim().to_lowercase() == "y" {
        rmls::remove(filename);
    }
}
