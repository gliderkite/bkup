# bkup

**[WIP]** A simple utility to backup files.

[![Build Status](https://travis-ci.com/gliderkite/bkup.svg?token=KzGLQfTbGDZSnqr7k9KT&branch=master)](https://travis-ci.com/gliderkite/bkup)


## How to use

Use cargo to build and run.

The following command will compare the *source*
directory with the *destination* directory, and update each file in the destination
that does not exist or it's older than the corresponding file in the source directory.

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

If you wish to ignore specific files or folder, it is possible to define a
*`.gitignore`-like* file in the same path of the executable which name must be
`.bkignore`.


## Roadmap

- [X] Basic backup implementation: source to destination for older files (*one way*).
- [X] Unit and integration Tests for main functionalities.
    - [ ] Parse YAML/JSON to recreate filesystem structure for better tests.
- [X] Integrate with CI pipeline.
- Parallel/Concurrent exploration (and backup):
    - [ ] Parallel: threads for each exploration.
    - [ ] Concurrent: async runtime (tokio) with pool of threads and reactor.
- Configuration:
    - [X] YAML CLI clap commands.
    - [X] Add accuracy parameter to take into account different filesystems.
    - [ ] *Daemonize* process to run in background.
    - [ ] Keep alive background process and backup every N seconds.
    - [ ] Read JSON configuration with multiple sources and destinations.
    - [ ] Option to backup destination into source (*round trip*).
    - [X] Ignore files and folder to backup with global *`.gitignore`-like* file.
        - [ ] Pass `.ignore` file path via CLI option.
- [ ] Create 2 binaries: export lib public functionalities + executable
