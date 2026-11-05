# JCS vectors

`doubles.tsv` holds 2,000 stratified double-precision numbers: powers of 2, the smallest and largest
subnormals and normals, neighbours of 1e21, 1e-7 and other rounding edges, `MAX_VALUE`, `MIN_VALUE`,
`-0`, decades, and random mantissas. Each line is the 16 hex digits of the IEEE 754 bit pattern, a
tab, and `String(x)` as Node computed it. The file header names the Node version and the seed.

Regenerate with `node gen_doubles.mjs` (Node v26.7.0, seed 8785). A larger one-off sample:
`node gen_doubles.mjs --count 100000 --seed <n> --random-only --out <file>`.

The RFC 8785 Appendix B number table and the section 3.2.3 sorting sample are literals in
`../../guard_jcs.rs`.
