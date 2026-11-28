# #207 — Mutation testing with cargo-mutants

## What is built in this change
- `mutants.toml`: scoped to `src/**/*.rs`, excludes `src/test.rs`, and documents
  the equivalent mutants that are excluded with a justification.
- `Makefile` target `mutants` so the run is reproducible locally.
- `.github/workflows/mutants.yml`: weekly scheduled job that runs
  `cargo mutants` and uploads `mutants.out/` as an artifact.

## What remains
1. Run `cargo mutants --config mutants.toml` on a machine with network access
   (the sandbox has none) and collect `mutants.out/survivors.txt`.
2. Triage each survivor in `access_control.rs`, `validation.rs`,
   `identity_map.rs`, `nft_mint.rs`:
   - non-equivalent → add a unit test in `src/test.rs` that kills it;
   - equivalent → add a `exclude_re` entry in `mutants.toml` with a comment
     explaining why no test can distinguish the mutant.
3. Re-run until `survivors.txt` contains only excluded/equivalent mutants.
4. Add the new tests to `src/test.rs` (one test per killed survivor, named
   `test_mutants_<module>_<behaviour>`), keeping `make check` green.

## Files to change
- `src/test.rs` (new tests)
- `mutants.toml` (exclusions for equivalent mutants)

## Done when
- `cargo mutants --config mutants.toml` reports zero non-equivalent survivors
  in the four modules listed in the acceptance criteria.
- Every killed survivor has a corresponding test.
- The weekly CI job uploads the report and fails on new survivors.
