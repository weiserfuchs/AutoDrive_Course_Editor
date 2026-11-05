# Brief

Repository commit: e4fe43e93f225e8c6d7635d62fb8cfdfd845a971 (short: e4fe43e)

## Questions

1. (how) How is an AutoDrive config XML file read into memory: which elements and attributes are parsed, and how do they become objects?
2. (how) How is the in-memory road network written back to a config XML file (element order, number formatting, encoding)?
3. (how) How is the road network represented in memory: node, connection, marker, group and flag types, and the structures that hold and index them (incl. KDTree)?
4. (how) How does the code tell FS19, FS22 and FS25 config files apart, and what differs in their handling?
5. (how) How is this code tested?

## Budgets

- File budget per question: 15 files
- History budget: 10 commits
- Validation: normal

## Paths left out of every search

- _bmad-output/
- _bmad/
- .claude/
- .agents/
- target/
- AGENTS.md
- CLAUDE.md
