# venvpruner

`venvpruner` is a command-line tool to search for and delete large Python virtual environments on your system. It allows you to free up disk space by cleaning up virtual environments that are no longer in use.

## Features

- Searches for all Python virtual environments on your system.
- Displays the size of each virtual environment.
- Allows you to select multiple virtual environments to delete.
- Confirms before deletion.
- Shows progress while deleting.
- Provides information on the total space reclaimed after cleanup.

## Installation

To use `venvpruner`, you need to have Rust installed on your system. If Rust is not installed, you can follow the instructions [here](https://www.rust-lang.org/tools/install).

Clone the repository and build the binary:

```bash
git clone https://github.com/yourusername/venvpruner.git
cd venvpruner
cargo build --release
```

## Usage

After building the binary, you can run `venvpruner` from the command line:

```bash
./target/release/venvpruner
```

This will scan for all virtual environments, display their sizes, and allow you to choose which ones to delete.

### Options

- No additional options or flags are needed. The tool will guide you through the process interactively.

## Example

```
$ venvpruner
Searching for virtual environments...
Found 5 virtual environments.
Total size of all virtual environments: 2.5 GB
Select the virtualenvs to delete:
1. ./venv1 (800 MB)
2. ./venv2 (600 MB)
3. ./venv3 (500 MB)
4. ./venv4 (400 MB)
5. ./venv5 (200 MB)
```

You can then select the virtual environments you want to delete, confirm the deletion, and view the reclaimed space.

## License

This project is licensed under the MIT License.
