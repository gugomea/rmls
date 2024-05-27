use std::{fs::File, io};

use rmls::ZombieFile;

const INPUT_MSG: &'static str = "Expected Input: <device> <output file>";

fn main() {
    let mut args = std::env::args();
    if args.len() != 3 {
        println!("{}", INPUT_MSG);
        return;
    }
    let device = args.nth(1).unwrap();
    let output_name = args.next().unwrap();
    let zombie_files = rmls::files_from_db();
    println!("Files:");
    let print_name = |t: (usize, &ZombieFile)| println!("{:?}", t);
    zombie_files
    .iter()
    .enumerate()
    .for_each(print_name);

    let mut index = String::with_capacity(5);
    io::stdin().read_line(&mut index).expect("Error reading input");
    let Ok(index) = index.trim().parse() else {
        println!("Expected number >= 0, got: {}", index.trim());
        return
    };
    let Some(file) = zombie_files.get::<usize>(index) else {
        println!("Index out of bounds: Len = {}; idx = {}", zombie_files.len(), index);
        return
    };

    let output = File::create(output_name).unwrap();
    rmls::recover_file(file, device, output);
}
