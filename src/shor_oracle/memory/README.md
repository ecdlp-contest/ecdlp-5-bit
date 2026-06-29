# Baseline Notes

This baseline emits a direct reversible lookup for the variable-base 5-bit Shor
ECDLP oracle:

```text
|a>|b>|P>|Q>|0> -> |a>|b>|P>|Q>|aP + bQ>|P+Q>|2P>
```

It is intentionally simple. Useful improvements should reduce CCX count,
logical qubits, or both while preserving the ABI, point-operation checks, and
trusted validation gate.
