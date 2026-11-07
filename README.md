# cats-grid

Rust remake of the "cats in the colored grid" puzzle for the G760. No ads, no network.

Rules: place one cat in every row, every column and every color region. Cats may not
touch each other, not even diagonally.

## Play

Launch **Cats Grid** from the application menu, or run
`/data/rust/cats-grid/target/release/cats-grid`.

- **Left click** — place a cat. A wrong cat costs a life; three wrong cats ends the run.
- **Right click** — mark a cell you ruled out (click again to clear).
- **Hint** — explains the next cat, not just where it goes. The card under the board
  names the deduction: a row, column or colour down to its last free square, or a
  chain of forced moves ("a cat in column 1 instead would force column 2 to take row
  4, which leaves the mint region with no square left"). Optional; the button is
  always available but never forced on you.
- **Extra life** — +1 life, usable mid-run or after a game over (max 5).
- **Clear marks** — wipe the board and start the same puzzle over.
- **New game** — new puzzle.

## Win tracking

`~/.local/share/cats-grid/stats.json`

- **Clean wins** — no hint and no extra life used during that run.
- **assisted** — wins where either was used. Counted separately, never added to clean.

Using a hint or buying a life marks the current run as assisted immediately; clearing
marks or restarting the app does not reset that flag for the run in progress.

## Build

```fish
cd /data/rust/cats-grid
cargo build --release
cargo test --release      # puzzle rules + generator uniqueness
./target/release/cats-grid --selftest   # 10 seeds, generation timing
```

Generation plants a legal placement, then nudges region borders until the solver can
prove that placement is the only solution — typically 1–25 ms per puzzle.
