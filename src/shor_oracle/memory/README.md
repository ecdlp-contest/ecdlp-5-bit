# Scalar-log table oracle

This submission replaces the baseline `A=aG`, `B=bQ`, point-add scratch flow with a scalar intermediate flow:

```text
Q = kG
p = b*k mod 21
R = (a+p)G = aG + bQ
```

The circuit still uses reversible exact-match tables, but it stores only two 5-bit scratch scalars (`k` and `p`) instead of two 11-bit affine points. This also replaces the 22-bit point-addition table with 10-bit scalar/product result tables.

Validation result from the trusted local harness:

- 9024 Fiat-Shamir shots OK
- input preservation OK
- phase cleanliness OK
- ancilla cleanup OK
- qubits: 52
- CCX / Toffoli: 37,128
- Toffoli depth: 37,128
- score: 1,930,656
