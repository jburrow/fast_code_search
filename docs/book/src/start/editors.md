# Editors

`fcs` prints `path:line:col:text` when its output is not a terminal, which
is the format editors' quickfix and "go to file:line" features read.

## Vim and Neovim

```vim
" one-off: results as a quickfix list
:cexpr system('fcs --format vimgrep ' . shellescape('fn main'))

" make :grep use it
set grepprg=fcs\ --format\ vimgrep\ --absolute
set grepformat=%f:%l:%c:%m
```

Then `:grep 'fn main'`, `:copen`. `--absolute` maps the workspace-relative
paths back to absolute ones through your configuration, so the list works
from any working directory.

## VS Code

Run `fcs` in the integrated terminal: `path:line:col` output is clickable.
With `--absolute` the links resolve regardless of the folder you opened.
The repository also contains a
[VS Code extension](https://github.com/jburrow/fast_code_search/tree/main/vscode-extension)
that talks to the server directly.

## Emacs

`M-x grep` with `fcs --format vimgrep --absolute QUERY` produces a
compilation buffer whose entries jump to the file and line.

## Shell

```bash
fcs -l TODO | xargs $EDITOR           # open every file containing TODO
fcs -l 'fn main' | fzf | xargs $EDITOR
fcs --json 'parse_query' | jq '.results[] | [.file_path, .line_number]'
```

Exit status follows grep: 0 when there were matches, 1 when none, 2 on
error, so `fcs -q pattern && …` works in scripts.
