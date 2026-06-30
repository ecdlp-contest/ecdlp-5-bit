# Baseline Notes

This baseline emits a direct reversible lookup for the variable-base 5-bit Shor
ECDLP oracle plus the selector-based `F_31`, `F_13`, and `F_11` field-witness ABI:

```text
|a>|b>|P>|Q>|field inputs>|0> -> |a>|b>|P>|Q>|field inputs>|aP + bQ>|P+Q>|2P>|field witnesses>
```

It is intentionally simple. Useful improvements should reduce CCX count,
logical qubits, or both while preserving the ABI, point-operation checks, and
trusted field-arithmetic validation gate.
