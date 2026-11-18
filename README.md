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
- Each question has a **Type**: Single answer, Multiple answers, or True / false.
  A true/false question always has the two options True and False. Its options
  are fixed while the type is True / false. Switch the type to edit them.

### Formulas

Prompts, answer options and explanations can contain formulas written in LaTeX.
Put inline formulas between single dollar signs, like `$x^2 + 1$`, and display
formulas between double dollar signs, like `$$\int_0^1 x\,dx$$`. Write `\$` for
a dollar sign. In the paste format, a `*` inside a formula is multiplication,
so only a `*` after the formula marks a correct answer, like `A) $2*x$ *`.

A `$` with no closing partner is shown as plain text, and the editor warns about
it. A formula that cannot be drawn is shown as written, and the editor warns
about it when you save.

## Notes

The app has no hand-written JavaScript. Two exceptions: a short bootstrap
module that `crates/web/src/api.rs` generates at runtime to start the database
worker, because browsers only start workers from scripts; and KaTeX, the library
that draws formulas. KaTeX is vendored in `crates/web/public/katex` (MIT
licence, see the `LICENSE` file there) and loaded by `crates/web/index.html`,
so formulas work offline.

## Sample quizzes

The worker can ship built-in sample quizzes. None are included yet. To add them,
put the JSON packs in `samples/` and list them in `SAMPLES` in
`crates/worker/src/lib.rs`.

## Writing questions as text

In the editor, open **Import questions from text** and paste questions in
the format below. The same rules appear under **Show the format** in the app,
where the examples are checked by a test, so they stay accurate.

### The three question types

Single answer, with exactly one `*`:

```
1. What is the powerhouse of the cell?
A) Nucleus
B) Mitochondria *
C) Ribosome
Explanation: Mitochondria make ATP.
```

Multiple answers, with `(multi)` on the question line and at least two `*`:

```
2. Pick the primary colours (multi)
a) Red *
b) Green
c) Blue *
d) Yellow
```

True or false, with `(true-false)` on the question line and an `Answer:` line.
Do not write option lines; True and False are added for you:

```
3. The Earth orbits the Sun. (true-false)
Answer: True
```

`Answer:` can be `True` or `False`, in any letter case, and can go before or
after `Explanation:`.

### Rules

- A question starts with a number and `.` or `)`.
- Options start with a letter and `)` or `.`, one per line.
- Put `*` at the end of each correct option. A single-answer question needs
  exactly one. A `(multi)` question needs at least two.
- For an option whose text is a star, write `\*` at the end. A lone `*` is the
  option's text.
- `Explanation:` is optional. Lines after it continue the explanation.
- A question can't be both `(multi)` and `(true-false)`.

### Formulas

Prompts, options and explanations can contain LaTeX formulas. Inline formulas go
between single dollar signs, like `$x^2 + 1$`. Display formulas go between
double dollar signs, like `$$\int_0^1 x\,dx$$`. Each formula must close on the
same line as its opening sign, except a display formula, which can run over
several lines.

- Write `\$` for a dollar sign that is not part of a formula.
- Inside a formula, `*` is multiplication. Only a `*` after the formula marks a
  correct answer, like `A) $2*x$ *`.
- A `$` with no closing partner is shown as plain text, and the editor warns
  about it.
- A formula that can't be drawn is shown as written, and the editor warns about
  it when you save.

### Why the rules are like this

The rules are deliberately few, so a single pass over the text is enough to
find every question. A `*` inside a formula, or a line inside a display formula
that starts with a number or a letter and `)`, is never read as a question or
option, because formulas often contain those characters.

The quiz's title, description, category, tags, time limit, pass mark, and
shuffle settings are set in the form, not in the text. Errors are listed with
line numbers. Parsing lives in `crates/domain/src/text_format.rs` and is
covered by `cargo test`.

## Continuous integration and GitHub Pages

- `.github/workflows/ci.yml` runs on every push and pull request. It runs the
  domain and database tests, then builds the worker and the app.
- `.github/workflows/pages.yml` runs on pushes to `main`. It runs the tests,
  builds the app with the repository name as its base path, and deploys it to
  GitHub Pages.

To turn on deployment, open the repository's **Settings → Pages** and set
**Source** to **GitHub Actions**. The site is then published at
`https://<user>.github.io/<repo>/`.
