# Baseline Notes

This baseline emits a direct reversible lookup for the variable-Q 5-bit Shor
ECDLP oracle:

```text
|a>|b>|Q>|0> -> |a>|b>|Q>|aG + bQ>
```

It is intentionally simple. Useful improvements should reduce CCX count,
logical qubits, or both while preserving the ABI and trusted validation gate.
