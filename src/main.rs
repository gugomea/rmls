use std::io::{self, Write};
use std::fs::File;
use std::path::PathBuf;
use rmls::ZombieFile;

const INPUT_MSG_RECOVER: &'static str = "Expected Input: <device> <output file>";
const USAGE: &'static str = "USAGE:\n1. rm <file1> <file2> <file3> ..\n2. rm --recover <device> <output>\n3. rm --tui (For interactive selection of files)";


fn recover(device: String, output_name: String) {
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

fn main() -> std::io::Result<()> {
    // --recover --tui
    let mut args = std::env::args();
    let files_to_remove = match args.nth(1) {
        Some(m) if m.trim().eq("--recover") => {
            let (Some(dev), Some(output)) = (args.next(), args.next()) else {
                println!("{}", INPUT_MSG_RECOVER);
                return Ok(());
            };
            recover(dev, output);
            return Ok(())
        }
        Some(m) if m.trim().eq("--tui") => {
            let Some(dir_name) = args.next() else {
                println!("Expected: <directory>");
                return Ok(());
            };
            rmls::tui(dir_name)?
        }
        Some(file) => {
            args.into_iter()
                .chain([file])
                .map(|x| PathBuf::new().join(x))
                .collect()
        }
        None => {
            println!("{}", USAGE);
            return Ok(());
        }
        
    };
    println!("Files: {:?}", files_to_remove);
    print!("Delete Contents[Y/n] "); io::stdout().flush().unwrap();
    let mut y_n = String::with_capacity(5);
    io::stdin().read_line(&mut y_n).expect("Error reading input");
    if y_n.trim().is_empty() || y_n.trim().to_lowercase() == "y" {
        files_to_remove
            .into_iter()
            .for_each(rmls::remove);
    }
    Ok(())
}
