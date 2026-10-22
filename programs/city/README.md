# deadroll_city

Program id `2Cr5vB5awqDVx6XRcxtFFYQEG6Zy3aSAcKcqzh1xbVjS`.

City, buildings, interiors and the order board (docs/ARCHITECTURE.md, 3.1; design in docs/CITY_DESCRIPTION.md). Not started yet: the program is an empty skeleton (`src/lib.rs` with no instructions) that keeps its id deployed.

## Build and test

```
anchor build --tools-version v1.54 -p deadroll_city
cargo test -p deadroll-city
```

Program tests go to `tests/` (see `programs/character/tests` for the pattern) once instructions exist.
