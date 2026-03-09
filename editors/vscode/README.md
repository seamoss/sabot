# Sabot Language - VS Code Extension

Syntax highlighting for the [Sabot](../../) programming language.

## Install (local)

### Option 1: Symlink

```bash
ln -s "$(pwd)" ~/.vscode/extensions/sabot-lang
```

Then reload VS Code (`Cmd+Shift+P` → "Reload Window").

### Option 2: Copy

```bash
cp -r . ~/.vscode/extensions/sabot-lang
```

## Features

- Syntax highlighting for `.sabot` files
- Comment toggling (`Cmd+/`)
- Bracket matching and auto-closing
- Highlights: comments, strings (with interpolation), numbers, symbols, keywords, word definitions, named stacks, all builtins, operators
