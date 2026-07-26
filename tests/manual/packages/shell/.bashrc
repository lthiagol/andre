# ~/.bashrc - Bash configuration

# History
HISTFILE=~/.bash_history
HISTSIZE=10000
shopt -s histappend

# Colors
export CLICOLOR=1
export LSCOLORS=ExFxBxDxCxegedabagacad

# Prompt
PS1='\u00ce\u03b2\u03b9\u03b1:\\w$ '

# Aliases
alias ll='ls -la'
alias la='ls -a'
alias ..='cd ..'
alias ...='cd ../..'