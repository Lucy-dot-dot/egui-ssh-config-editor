# egui-ssh-config

A graphical SSH config file editor built with Rust and egui.

## Features

- Visual interface for editing SSH config files
- Support for Include directives and multi-file configurations
- Search and filter host entries
- Circular include detection
- Dirty state tracking with save prompts
- Quick addition of legacy SSH options for older servers
- Always-on-top mode
- Keyboard shortcuts for common operations

## Building

Requires Rust toolchain. Build with:

```bash
cargo build --release
```

## Usage

The application automatically loads `~/.ssh/config` on startup if it exists. You can open other config files via the File menu.

### Keyboard Shortcuts

- `Ctrl+O` - Open SSH config file
- `Ctrl+S` - Save all changes
- `Ctrl+N` - New host entry
- `Ctrl+Q` - Quit (prompts to save if there are unsaved changes)
- `Ctrl+F` - Focus search box
- `Ctrl+A` - Toggle always on top
- `Ctrl+Shift+L` - Add legacy SSH options to selected host
- `Escape` - Clear search / unfocus

### Legacy SSH Options

The `Ctrl+Shift+L` shortcut adds the following options to support older SSH servers:

- `HostKeyAlgorithms +ssh-rsa,ssh-rsa-cert-v01@openssh.com,ssh-dss`
- `PubkeyAcceptedAlgorithms +ssh-rsa,ssh-rsa-cert-v01@openssh.com`
- `Ciphers +aes256-cbc,aes128-cbc,3des-cbc`
- `MACs +hmac-sha1,hmac-md5`
- `KexAlgorithms +diffie-hellman-group14-sha1,diffie-hellman-group1-sha1`

## Multi-file Support

The editor fully supports SSH config files that use Include directives. Changes to host entries are saved back to their original source files, preserving your config file structure.

## License

Dual-licensed under MIT or Unlicense. Choose whichever you prefer. Attribution is appreciated but not required.
