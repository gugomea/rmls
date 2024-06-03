# rmls. (Maybe) recover your files.
Uses fiemap ioctl with the `fiemap` crate to retrieve the extents(blocks of contiguous blocks of disk memory) of a given file. To achieve this, we save necessary metadata of file before deleting, so that we can retrieve it later and (hopefully, if the memory in disk has not been overwritten) recover the file. It's not a replacement of a bin directory, since this really unlinks the file from the OS. Your memory might be corruped in minutes or in days, so try to use it fast :)
Since it uses extents, only Ext4 is suported.
# Features


* Delete files/directories
* Recover any deleted file with this tool
* Terminal User Interface to remove files
    * move up and down with `k` and `j`
    * Open directory `space`
    * Select more than one file with `shift + v`
    * Undo last operation with `u`
To facilitate tree traversal, the directories have unique identifiers following the pattern:
```
//.                        [0]           (not visible)
// ├── Cargo.lock          [0, 0]         => Node { id: [0, 0], name: "Cargo.lock", open: flase, cached_children: None }
// ├── Cargo.toml          [0, 1]
// ├── files2.sh           [0, 2]
// ├── logs.txt            [0, 3]
// ├── src                 [0, 4]         => Node { id: [0, 4], name: "Cargo.lock", open: true, cached_children: Some([..]) }
// │   ├── bin             [0, 4, 0]
// │   │   ├── recover.rs  [0, 4, 0, 1]
// │   │   ├── rm.rs       [0, 4, 0, 1]
// │   │   └── tui.rs      [0, 4, 0, 3]
// │   └── lib.rs          [0, 4, 1]
// └── T                   [0, 5]
```
Heach file has Rc<RefCell<Node>> to the children and a weak reference to the parent. 

## Demo

![This is an alt text.](https://github.com/gugomea/rmls/blob/main/demo.gif)


## Commands

Delete listed files/directories
```
rmls <file1> <file2> <file3> ...
```

Delete files interacitvely with the terminal interface
```
rmls --tui <directory>
```

Recover the files(It will prompt you which file to recover)
```
rmls --recover <device where partition is mounted> <output_name>
```
