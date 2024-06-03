# rmls. (Maybe) recover your files.
Uses fiemap ioctl with the `fiemap` crate to retrieve the extents(blocks of contiguous blocks of disk memory) of a given file. To achieve this, we save necessary metadata of file before deleting, so that we can retriee it later and (hopefully, if the memory in disk has not been overwritten) recover the file. It's not a replacement of a bin directory, since this really unlinks the file from the OS. Your memory might be corruped in minutes or in days, so try to use it fast :)
Since it uses extents, only Ext4 is suported.
# Features


* Delete files/directories
* Recover any deleted file with this tool
* Terminal User Interface to remove files
    * move up and down with `k` and `j`
    * Open directory `space`
    * Select more than one file with `shift + v`
    * Undo last operation with `u`

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
