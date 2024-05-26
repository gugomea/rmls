use std::{fs::{metadata, File}, io::{self, Read, Seek, SeekFrom, Write}, time::Instant};
use fiemap::{fiemap, FiemapExtent};
use serde::{Deserialize, Serialize};

const BLOCK_SIZE: u64 = 4096;
const DB: &'static str = "DB.bin";

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct ZombieFile {
    name: String,
    len: usize,
    extents: Vec<Extent>,
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
            Err(e) =>  {
                match *e {
                    bincode::ErrorKind::Io(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    err => panic!("Unespected error while writting:\n{:?}", err),
                }
            }
        }
    }
}

fn persist_zombie_file<A: AsRef<str>, B: AsRef<str>> (file: ZombieFile, device: A, name: B) {
    let mut device = File::open(device.as_ref()).expect("Error opening device");
    let mut length = file.len as u64;
    let mut buff = [0; 100 * BLOCK_SIZE as usize];
    let mut output = File::create(name.as_ref()).expect("Error creating output file");
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

fn get_input() -> (String, String) {
    let mut args = std::env::args();
    if args.len() != 3 {
        println!("Expected Input: <device> <input file>");
        panic!("Real Input: {:?}", args);
    }
    (
        args.nth(1).expect("Expected device name"),
        args.next().expect("Expected filename")
    )
}

fn main() {
    let (device, filename) = get_input();
    let fiemap = fiemap(&filename)
        .expect("FIEMAP FAILED")
        .filter_map(|x| match x {
            Ok(x) => Some(Extent::from(x)),
            Err(err) => panic!("last OS error: {err:?}"),
        }).collect();

    //WRITE FILE METADATA TO DATABASE.
    let instant = Instant::now();
    {
        let metadata = metadata(&filename).expect("Error reading metadata from file");
        let zombie = ZombieFile { name: filename, len: metadata.len() as usize, extents: fiemap };
        let mut db = File::create(DB).expect("Error opening DB");

        write_zombie_file(&mut db, zombie);
    }
    println!("Write file metadata to database: {:?}", instant.elapsed());

    //READ ZOMBIE FILE.
    let instant = Instant::now();
    let zombie = {
        let mut db = File::open(DB).unwrap();
        let mut buff = vec![];
        db.read_to_end(&mut buff).expect("TODO: BETTER");
        read_zombie_file(&buff)
    };
    println!("Load zombie file to memory: {:?}", instant.elapsed());

    //PERSIST FILE TO FILESYSTEM
    let instant = Instant::now();
    {
        persist_zombie_file(zombie, &device, "new_file.bin");
    }
    println!("Persis zombie file to `new_file.bin`: {:?}", instant.elapsed());

}
