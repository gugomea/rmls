use std::{fs::{canonicalize, metadata, remove_dir_all, remove_file, File, Metadata, OpenOptions}, io::{self, Read, Seek, SeekFrom, Write}, path::Path, time::Instant, usize};
use fiemap::{fiemap, FiemapExtent};
use serde::{Deserialize, Serialize};

const BLOCK_SIZE: u64 = 4096;
const DB: &'static str = "DB.bin";
const INPUT_MSG: &'static str = "Expected Input: <device> Option<input file>";

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ZombieFile {
    name: String,
    len: usize,
    extents: Vec<Extent>,
}

impl ZombieFile {
    fn new(name: String, m: Metadata, extents: Vec<Extent>) -> Self {
        Self {
            name,
            len: m.len() as usize,
            extents,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
struct Extent {
    start: u64,
    len: u64,
}

impl From<FiemapExtent> for Extent {
    fn from(value: FiemapExtent) -> Self {
        Extent {
            start: value.fe_physical,
            len: value.fe_length,
        }
    }
}

fn write_zombie_file<W: io::Write>(writer: &mut W, file: ZombieFile) {
    while let Err(err) = bincode::serialize_into(&mut *writer, &file) {
        match *err {
            bincode::ErrorKind::Io(e) if e.kind() == io::ErrorKind::Interrupted => {
                println!("Interrupted: {:?}", e);
            }
            err => panic!("Unespected error while writting:\n{:?}", err),
        }
    }
}

fn read_zombie_file(data: &[u8]) -> ZombieFile {
    loop {
        match bincode::deserialize_from(data) {
            Ok(v) => break v,
            Err(err) =>  {
                match *err {
                    bincode::ErrorKind::Io(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    err => panic!("Unespected error while writting:\n{:?}", err),
                }
            }
        }
    }
}

fn persist_zombie_file<A: AsRef<str>, W: Write> (file: ZombieFile, device: A, mut output: W) {
    let mut device = File::open(device.as_ref()).expect("Error opening device");
    let mut length = file.len as u64;
    let mut buff = [0; 100 * BLOCK_SIZE as usize];
    for extent in file.extents {
        let offset = extent.start;
        device.seek(SeekFrom::Start(offset)).expect("Error offseting block");

        let mut bytes_to_read = match length.checked_sub(extent.len) {
            Some(n) => {
                length = n;
                extent.len
            }
            None => length,
        };
        while bytes_to_read > 0 {
            let Ok(just_read) = device.read(&mut buff) else {
                continue;
            };
            let read = match bytes_to_read < just_read as u64 {
                true => bytes_to_read as usize,
                false => just_read,
            };
            let mut written = 0;
            while written < read {
                match output.write(&buff[written..read]) {
                    Ok(w) => written += w,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    err => panic!("Unespected error while writting:\n{:?}", err),
                }
            }
            bytes_to_read -= read as u64;
        }
    };
}

fn get_input() -> (String, Option<String>) {
    let mut args = std::env::args();
    (
        args.nth(1).expect(INPUT_MSG),
        args.next()
    )
}

fn append_file_db<P: AsRef<Path>>(filename: P) {
    let fiemap: Vec<_> = fiemap(&filename)
        .expect("FIEMAP FAILED")
        .filter_map(|x| match x {
            Ok(x) => Some(Extent::from(x)),
            Err(err) => panic!("last OS error: {err:?}"),
        }).collect();
    let metadata = metadata(&filename)
        .expect("Error reading metadata from file");
    if metadata.is_file() {
        // APPEND THE EXTENT INFORMATION INTO THE DB
        println!("Adding file: {:?}", filename.as_ref());
        let zombie = ZombieFile::new(filename.as_ref().to_str().unwrap().to_owned(), metadata, fiemap);
        let mut db = OpenOptions::new()
            .create(true).append(true)
            .open(DB).expect("Error opening DB");
        write_zombie_file(&mut db, zombie);
        return;
    }
    for entry in std::fs::read_dir(filename).expect("path probably doesn't exist") {
        let path = entry.unwrap().path();
        append_file_db(path.to_str().unwrap().to_owned());
    }
}

fn main() {
    let (device, filename) = get_input();
    if let Some(filename) = filename {
        let instant = Instant::now();
        let m = metadata(&filename).unwrap();
        let filename = canonicalize(filename).unwrap();
        append_file_db(&filename);
        println!("Write file metadata to database: {:?}", instant.elapsed());
        if m.is_file() {
            remove_file(filename).expect("Couldn't remove file");
        } else {
            remove_dir_all(filename).expect("Couldn't remove directory");
        }
        return;
    }
    //READ DATABASE
    let instant = Instant::now();
    let mut db_data = {
        let mut db = File::open(DB).unwrap();
        let mut buff = vec![];
        db.read_to_end(&mut buff).expect("TODO: BETTER");
        let mut zombie_files = vec![];
        let mut i = 0;
        while i < buff.len() {
            let curr = read_zombie_file(&buff[i..]);
            i += bincode::serialized_size(&curr)
                .expect("Error serializing zombie_file") as usize;
            zombie_files.push(curr);
        }
        zombie_files
    };
    println!("Read extent metadata from DB: {:?}", instant.elapsed());
    println!("CHOOSE THE INDEX OF THIS ARRAY:");
    let show_file = |(i, d): (usize, &ZombieFile)| println!("{i}.\t{}", d.name);
    db_data.iter()
        .enumerate()
        .for_each(show_file);
    let mut index = String::with_capacity(5);
    io::stdin().read_line(&mut index).expect("Error reading input");
    let Ok(index) = index.trim().parse() else {
        println!("Expected: <number >= 0>, real input: <{}>", index.trim());
        return;
    };
    if index >= db_data.len() {
        println!("GOT: {}, but db size is : {}", index, db_data.len());
    }
    let selected_zfile = db_data.swap_remove(index);

    let instant = Instant::now();
    let new_file = File::create("new_file.bin").expect("Error creating output file");
    {
        persist_zombie_file(selected_zfile, &device, new_file);
    }
    println!("Persis zombie file to `new_file.bin`: {:?}", instant.elapsed());
}
