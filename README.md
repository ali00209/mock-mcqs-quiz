# MockTest

Offline quiz and mock-exam app. Quizzes, attempts, and history are stored in a
SQLite database inside the browser (OPFS), so everything works without a server.

## Layout

- `crates/domain` – data types, validation, scoring, and the message protocol
- `crates/db` – SQLite schema and queries (has unit tests)
- `crates/worker` – wasm module that runs the database inside a Dedicated Worker
- `crates/web` – the user interface, written in Rust with Leptos and built with Trunk
- `scripts/` – build and serve helpers

## Setup

1. Install Rust with [rustup](https://rustup.rs). The `rust-toolchain.toml` file
   installs the stable toolchain and the wasm target automatically.
2. Install the wasm-bindgen CLI and Trunk:
   ```
   cargo install wasm-bindgen-cli --version 0.2.129
   cargo install trunk
   ```

## Run

```
scripts/build-worker.sh   # compile the database to wasm (run once, and after changing crates/worker)
scripts/serve.sh          # build the app with Trunk and open http://localhost:8080
```



## Test

```
cargo test -p mocktest-domain -p mocktest-db
```

The worker and the UI only build for wasm, so use `--target wasm32-unknown-unknown` (or Trunk) for them.

## Using the app

- **New quiz** creates an empty quiz. Add questions, mark the correct answers,
  and save. A quiz needs a title and at least one complete question before it
  can be started.
- **Start** begins an attempt. Attempts are saved as you answer, so you can
  close the tab and resume from the library.
- **Import / Export** read and write `.quiz.json` files.
- **Data** makes full backups and restores, and can reset everything.

## Notes

The app has no hand-written JavaScript. The one exception is a short bootstrap
module that `crates/web/src/api.rs` generates at runtime to start the database
worker, because browsers only start workers from scripts.

## Sample quizzes

The worker can ship built-in sample quizzes. None are included yet. To add them,
put the JSON packs in `samples/` and list them in `SAMPLES` in
`crates/worker/src/lib.rs`.

## Writing questions as text

In the editor, open **Import questions from text** and paste questions in
this format:

```
1. What is the powerhouse of the cell?
A) Nucleus
B) Mitochondria *
C) Ribosome
Explanation: Mitochondria make ATP.

2. Pick the primary colours (multi)
a) Red *
b) Green
c) Blue *
```

- A question starts with a number and `.` or `)`.
- Options start with a letter and `)` or `.`. Put `*` after each correct one.
- Add `(multi)` after a question with more than one correct answer.
- `Explanation:` is optional.

The quiz's title, description, category, tags, time limit, pass mark, and
shuffle settings are set in the form, not in the text. Errors are listed with
line numbers. Parsing lives in `crates/domain/src/text_format.rs` and is
covered by `cargo test`.
