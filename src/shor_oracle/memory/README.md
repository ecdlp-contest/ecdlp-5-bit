# Variable-base scalar-log oracle

The live track now validates the variable-base oracle:

```text
|a>|b>|P>|Q>|0> -> |a>|b>|P>|Q>|aP + bQ>|P+Q>|2P>
```

This submission changes strategy from affine point scratch tables to scalar-log intermediates. Because the trusted evaluator only supplies valid group points from the order-21 subgroup, five selected point bits `(x0, x2, x4, y3, y4)` uniquely identify each point as `kG`.

The circuit flow is:

```text
P = pG
Q = qG
ap = a*p mod 21
bq = b*q mod 21
R = (ap+bq)G
P+Q = (p+q)G
2P = (2p)G
```

Zero scalar-product rows are skipped because they would only XOR zero into scratch.

Trusted local validation:

- 9024 Fiat-Shamir shots OK
- input preservation OK
- oracle `aP+bQ` OK
- point-add `P+Q` OK
- point-double `2P` OK
- phase cleanliness OK
- ancilla cleanup OK
- qubits: 94
- CCX / Toffoli: 57,292
- Toffoli depth: 57,292
- score: 5,385,448
