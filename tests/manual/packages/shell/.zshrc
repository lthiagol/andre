# ~/.zshrc - Zsh configuration

# History
HISTFILE=~/.zsh_history
HISTSIZE=10000
SAVEHIST=10000
setopt HIST_IGNORE_DUPS

# Colors
export CLICOLOR=1
export LSCOLORS=ExFxBxDxCxegedabagacad

# Aliases
alias ll='ls -la'
alias la='ls -a'
alias ..='cd ..'
alias ...='cd ../..'

# Load additional configs
[ -f ~/.aliasrc ] && source ~/.aliasrc