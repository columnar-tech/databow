---
icon: lucide/download
---

<!--
Copyright 2026 Columnar Technologies Inc.
SPDX-License-Identifier: Apache-2.0
-->

# Installation

## PyPI

### uv

Install with [uv](https://docs.astral.sh/uv/):

```sh
uv tool install databow
```

### pipx

Install with [pipx](https://pipx.pypa.io/stable/):

```sh
pipx install databow
```

### pip

Install with [pip](https://pip.pypa.io/en/stable/):

```sh
pip install databow
```

## crates.io

Install from [crates.io](https://crates.io/crates/databow):

```sh
cargo install databow
```

## Homebrew

Install with [Homebrew](https://brew.sh/):

```sh
brew install columnar-tech/tap/databow
```

## Source

Install from source:

```sh
git clone https://github.com/columnar-tech/databow.git
cargo install --path databow
```

## Shell Completion

databow can complete CLI arguments and [connection profile](tutorial.md#connection-profiles) names when the `Tab` key is pressed. Follow the instructions for your shell below, then restart the shell. Run `echo $SHELL` to print your login shell.

### Bash

Enable shell completion for [Bash](https://www.gnu.org/software/bash/):

```sh title="~/.bashrc"
eval "$(databow generate-shell-completion bash)"
```

### Zsh

Enable shell completion for [Zsh](https://www.zsh.org/):

```sh title="~/.zshrc"
eval "$(databow generate-shell-completion zsh)"
```

### Fish

Enable shell completion for [fish](https://fishshell.com/):

```fish title="~/.config/fish/completions/databow.fish"
databow generate-shell-completion fish | source
```

### PowerShell

Enable shell completion for [PowerShell](https://learn.microsoft.com/powershell/):

```powershell title="$PROFILE"
(& databow generate-shell-completion powershell) | Out-String | Invoke-Expression
```
