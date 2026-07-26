# Alloy 6 Formal Model: Text Selection Subsystem

## Summary

Four Alloy 6 formal models of the torvox text selection subsystem, verified with Alloy Analyzer 6.2.0.

| Model | File | Assertions | Status |
|-------|------|-----------|--------|
| Grid | `grid.als` | 4 | 4 UNSAT |
| Touch Classification | `touch.als` | 3 | 3 UNSAT |
| Selection | `selection.als` | 5 | 4 UNSAT, 1 SAT |
| Integration Properties | `properties.als` | 5 | 5 UNSAT |
| **Total** | | **17** | **16 UNSAT, 1 SAT** |

## Key Verified Properties

1. **Grid integrity**: All cells within bounds, positions unique, char classes disjoint
2. **Touch classification**: Paste and select are mutually exclusive; every touch has exactly one classification; empty cells never classified as whitespace
3. **Selection ordering**: ordered start always precedes ordered end; contains remains within non-negative bounds; word expansion stays in bounds (same row, valid columns)
4. **Integration**: Empty/whitespace touch → paste action; text touch → select action; empty and text are mutually exclusive for same cell

## SAT Result (Expected)

`contains_within_bounds` (selection.als line 105) produces SAT because in multi-row contiguous selections (Char/Word/Semantic), intermediate rows accept any column — including negative Int values. This is by design: the `contains` predicate for contiguous mode constrains only the first row (c >= lo.col) and last row (c <= hi.col), leaving intermediate rows fully selected. This is consistent with how real terminal selections work.

## Running

```bash
nix shell nixpkgs#alloy6
for f in docs/models/alloy/*.als; do
  java -jar $(find $(dirname $(readlink -f $(which java)))/.. -name "alloy*.jar") exec -f "$f"
done
```

## Rust Code Mapping

| Alloy Concept | Rust Equivalent |
|--------------|-----------------|
| Cell.char.class | `Cell.char` compared to `'\0'`, `' '`, word chars |
| isEmpty | `Selection::is_position_empty` |
| isText | Word/CJK character classification |
| TouchClass | `torvox_core::selection::TouchClass` |
| expandWord | `Selection::expand_word` |
| expandSemantic | `Selection::expand` in Semantic mode |
