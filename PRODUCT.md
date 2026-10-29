# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users
Students practising for exams and teachers or trainers who write quizzes, equally. Students sit timed mock exams; teachers author the questions.

## Product Purpose
MockTest is an offline quiz and mock-exam app. It lets people build quizzes, take timed attempts, and review their history. Success means a user can run a realistic timed mock exam and learn from the result without needing an account or a server.

## Positioning
Quizzes, attempts, and history are stored in a SQLite database inside the browser (OPFS), so the app works with no server and no account. Quizzes are portable `.quiz.json` files, and questions can be written as plain text and imported with line-numbered error reports.

## Operating Context
- Attempts are saved as the user answers, so they can close the tab and resume from the library.
- Mock exams can use a time limit, a pass mark, and shuffled questions and options, all set in the quiz form.
- Full backups and restores are available from the Data screen.
- Quiz authors write questions in an editor, mark the correct answers, and can paste questions in the text format.

## Capabilities and Constraints
- Create, edit, duplicate, export, import, and delete quizzes; start attempts; view attempt history and reviews.
- Question kinds include single-answer and multi-answer (`(multi)`), with optional explanations.
- Data lives in the browser; there is no server-side storage.
- The UI is written in Rust with Leptos and built with Trunk. The database runs in a Dedicated Worker as wasm.
- The app has no hand-written JavaScript apart from a generated worker bootstrap.
- Sample quizzes are planned but not included yet; their final form is undecided.

## Brand Commitments
- The product name is MockTest.

## Evidence on Hand
No real user research, testimonials, benchmarks, or brand assets exist in the repository. Future work must not invent any of them.

## Product Principles
- Work fully offline; never require an account or server to take a quiz.
- Make a mock exam feel like the real thing: timing, pass marks, and shuffling are first-class.
- Keep quizzes portable: plain files that can be exported, shared, and re-imported.
- Authoring should be fast, including pasting questions as text.
