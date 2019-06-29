# bkup

A simple utility to backup files.

[![Build Status](https://travis-ci.com/gliderkite/bkup.svg?token=KzGLQfTbGDZSnqr7k9KT&branch=master)](https://travis-ci.com/gliderkite/bkup)


## How to use

Use cargo to build and run.

The following command will compare the *source*
directory with the *destination* directory, and update each file in the destination
that does not exist or it's older than the related file in the source directory.

```
cargo run --release -- update -s <source> -d <destination>
```

For a list of possible options run with `--help`:

```
USAGE:
    bkup [SUBCOMMAND]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    help      Prints this message or the help of the given subcommand(s)
    update    Update the destination folder according to its delta with the source folder
```


## TODO

- [X] Basic backup implementation: source to destination for older files (*one way*).
- [ ] Unit and integration Tests for main functionalities.
    - [ ] Parse YAML/JSON to recreate filesystem structure for better tests.
- [X] Integrate with CI pipeline.
- Parallel exploration (and backup):
    - [ ] Simple: 2 threads for each exploration.
    - [ ] Complex: pool of threads that pop from a queue tasks (explore directory or
        compare directories).
- Configuration:
    - [X] YAML CLI clap commands.
    - [ ] *Daemonize* process to run in background.
    - [ ] Keep alive background process and backup every N seconds.
    - [ ] Read JSON configuration with multiple sources and destinations.
    - [ ] Option to backup destination into source (*round trip*).
    - [ ] Ignore files and folder to backup (for each source/destination).
- [ ] Create 2 binaries: export lib public functionalities + executable
