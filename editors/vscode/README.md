# Sabo Language - VS Code Extension

Syntax highlighting for the [Sabo](../../) programming language.

## Install (local)

### Option 1: Symlink

```bash
ln -s "$(pwd)" ~/.vscode/extensions/sabo-lang
```

Then reload VS Code (`Cmd+Shift+P` → "Reload Window").

### Option 2: Copy

```bash
cp -r . ~/.vscode/extensions/sabo-lang
```

## Features

- Syntax highlighting for `.sabo` files
- Comment toggling (`Cmd+/`)
- Bracket matching and auto-closing
- Highlights: comments, strings (with interpolation), numbers, symbols, keywords, word definitions, named stacks, all builtins, operators
