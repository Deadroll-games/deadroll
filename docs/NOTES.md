# Notes

## Balance simulator

Biggest advantage of this architecture: the combat logic is written in Rust. Move it into a separate crate with no dependency on Anchor, and run the same code off-chain in Monte Carlo over millions of fights. This shows which builds break balance before deploying.
