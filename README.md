# bkup

**[WIP]** A simple utility to backup files.

[![Build Status](https://travis-ci.com/gliderkite/bkup.svg?token=KzGLQfTbGDZSnqr7k9KT&branch=master)](https://travis-ci.com/gliderkite/bkup)


## How to use

Use cargo to build and run.

The following command will compare the *source*
directory with the *destination* directory, and update each file in the destination
that does not exist or it's older than the corresponding file in the source directory.

```
RUST_LOG=info cargo run --release -- update -s <source> -d <destination>
```

At the moment, only the `update` subcommand is available. For a list of possible
options run with `--help`:

```
USAGE:
    bkup update [FLAGS] [OPTIONS] --destination <DESTINATION_PATH> --source <SOURCE_PATH>

FLAGS:
    -h, --help       Prints help information
    -i, --ignore     When set parse the .gitignore file of the source directories
    -V, --version    Prints version information

OPTIONS:
    -a, --accuracy <ACCURACY_MS>            Sets the accuracy in ms for a source file to be considered newer than its
                                            destination
    -d, --destination <DESTINATION_PATH>    Sets the path of the destination folder to update
    -s, --source <SOURCE_PATH>              Sets the path of the source folder
```

If you wish to ignore specific files or folders, you can set the `--ignore` flag
of the `update` subcommand. If this flag is set, every directory (both in source
and destination) will be parsed according to its `.gitignore` file (if any), and
every file and folder that matches the `.gitignore` entries will be ignored (as
if they didn't exist).

```
RUST_LOG=info cargo run --release -- update -s <source> -d <destination> --ignore
```

Please note that this may lead to unexpected results when the `.gitignore` file
in the source directory is not equal to the one in the destination directory.
For example, consider the scenario where the `.gitignore` file in the destination
directory contains an entry that does not exist in the source `.gitignore` file.
In this case the entry will be copied from source to destination independently
if it is actually newer. The idea is that this entry should not be ignored
anymore for the source, therefore it will be replicated in the destination.



## Roadmap

- [X] Basic backup implementation: source to destination for older files (*one way*).
- [X] Unit and integration Tests for main functionalities.
    - [ ] Parse YAML/JSON to recreate filesystem structure for better tests.
- [X] Integrate with CI pipeline.
- [ ] Parallel/Concurrent exploration (and backup):
    - [X] Parallel: thread per directory visit.
    - [ ] Concurrent: async runtime (tokio) (blocked on https://github.com/tokio-rs/tokio/issues/588).
- [ ] Configuration:
    - [X] YAML CLI clap commands.
    - [X] Add accuracy parameter to take into account different filesystems.
    - [ ] *Daemonize* process to run in background.
    - [ ] Keep alive background process and backup every N seconds.
    - [ ] Read JSON configuration with multiple sources and destinations.
    - [ ] Option to backup destination into source (*round trip*).
    - [X] Ignore files and folder to backup according to  `.gitignore` files.
- [ ] Create 2 binaries: export lib public functionalities + executable
