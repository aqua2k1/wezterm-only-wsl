---
tags:
  - multiplexing
---
# `default_domain = "local"`

{{since('20220319-142410-0fcdea07')}}

The Windows single-session build always selects a configured WSL domain and
starts one WSL shell through ConPTY. Set `config.default_domain` to the WSL
domain name when more than one WSL domain is configured.

For example, if:

```
; wsl -l -v
  NAME            STATE           VERSION
* Ubuntu-18.04    Running         1
```

then wezterm will by default create a `WslDomain` with the name `"WSL:Ubuntu-18.04"`
and if I set my config like this:

```lua
config.default_domain = 'WSL:Ubuntu-18.04'
```

then when the Windows GUI starts, it will open with a shell running inside
that Ubuntu distribution.
