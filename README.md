# rmls. (Maybe) recover your files.

# Features


* Delete files/directories
* Recover any deleted file with this tool
* Terminal User Interface to remove files
    * move up and down with `k` and `j`
    * Open directory `space`
    * Select more than one file with `shift + v`
    * Undo last operation with `u`

## Demo

![This is an alt text.](https://github.com/gugomea/rmls/demo.gif)


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
