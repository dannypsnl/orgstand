# OrgStand

A terminal-based TODO entry manager for org-mode files, written in Rust. Unlike traditional org-mode viewers that work with files, OrgStand focuses on **TODO entries** as the primary unit, making it easy to browse and manage tasks across multiple org files.

## Philosophy

OrgStand ignores file structure and presents all TODO entries from your org files in a flat list. When you focus on a TODO entry, it opens in a dedicated view where you can see its full content including sub-tasks, properties, and metadata.

This approach is inspired by tools like Logseq and Roam Research, where blocks/entries are first-class citizens rather than files.

## Features

- **TODO-Centric Browser**: Browse all TODO entries across multiple org files in one view
- **Entry-Focused Viewer**: View complete TODO entries with all their content
- **Built-in Editor**: Edit TODO entries directly in the TUI
  - Full text editing with standard keyboard controls
  - Save changes back to the original org file
  - Auto-save on exit
- **Quick Actions**:
  - Toggle TODO ↔ DONE with a single key (`t`)
  - Add SCHEDULED dates with visual calendar picker (`s`)
  - Add DEADLINE dates with visual calendar picker (`D`)
  - No manual date typing - use arrow keys to navigate!
  - All changes save directly to your org files
- **TODO States**: Color-coded TODO/DONE/NEXT/WAITING keywords
  - TODO: Red
  - DONE: Green
  - Others: Yellow
- **Full Content Display**:
  - Multi-level headings with different colors
  - Code blocks with borders
  - Links with underlines
  - Timestamps
  - Properties drawers
  - Lists
- **Keyboard Navigation**: Vim-style (j/k) and arrow keys
- **Scroll Support**: Navigate through long org files

## Installation

```bash
cargo build --release
```

## Usage

### Browse TODO entries in current directory
```bash
cargo run
# or after building:
./target/release/orgstand
```

### Browse TODO entries in a specific directory
```bash
cargo run -- /path/to/org/directory
# For example, with Beorg files:
cargo run -- ~/Library/Mobile\ Documents/iCloud~com~appsonthemove~beorg/Documents/org/
```

This will **recursively scan all `.org` files** in the directory and all subdirectories (up to 5 levels deep), extracting all TODO entries and presenting them in a unified list regardless of which file or folder they're in.

**Smart Scanning:**
- Automatically skips hidden directories (starting with `.`)
- Skips common large directories (`node_modules`, `target`, `.git`, etc.)
- Won't hang even if pointed at large directory trees
- Safe to use in your home directory with org files nested in various projects

## Keyboard Controls

### Browser Mode (TODO List)
- `↑`/`↓` or `k`/`j`: Navigate TODO entries
- `Enter`: Open/focus on selected TODO entry
- `q`: Quit

### Viewer Mode (TODO Entry View)
- `↑`/`↓` or `k`/`j`: Scroll through entry content
- `t`: Toggle between TODO and DONE states
- `s`: Add/update SCHEDULED date
- `D` (Shift+d): Add/update DEADLINE date
- `e`: Enter edit mode
- `Esc`: Back to TODO list
- `q`: Quit

### Date Input Mode (Calendar Picker)
- **Visual calendar** showing current month
- `←/→` or `h`/`l`: Move day by day
- `↑/↓` or `k`/`j`: Move week by week
- `<`/`>` or `Page Up`/`Page Down`: Change month
- `Enter`: Confirm selected date
- `Esc`: Cancel without saving
- Selected date shown as `[DD]`, today marked with `*`

### Editor Mode (Editing TODO Entry)
- Normal text editing keys (arrows, backspace, delete, etc.)
- `Esc` or `Ctrl+S`: Save changes and return to viewer mode
- All standard text editing works!
- Changes are automatically saved when you exit

## Color Scheme

- **Level 1 Headings**: Light Blue
- **Level 2 Headings**: Light Green
- **Level 3 Headings**: Light Yellow
- **Level 4 Headings**: Light Magenta
- **TODO Keywords**: Red (bold)
- **DONE Keywords**: Green (bold)
- **Links**: Blue (underlined)
- **Timestamps**: Magenta
- **Code Blocks**: Cyan
- **Properties/Drawers**: Dark Gray

## Examples

Browse all TODO entries in the example directory:
```bash
cargo run -- .
```

Browse your Beorg TODO entries:
```bash
cargo run -- ~/Library/Mobile\ Documents/iCloud~com~appsonthemove~beorg/Documents/org/
```

This will show all TODO entries from `inbox.org` and any other org files in that directory in a single unified view.

## Architecture

OrgStand demonstrates that you can work with org files without Emacs by:
1. Recursively scanning directories and subdirectories for all `.org` files
2. Parsing each file to extract TODO entries with their full content
3. Presenting entries in a flat, file-agnostic view
4. Using `ratatui` for the TUI interface

**Key Design Decisions:**
- **Entry-centric, not file-centric**: The primary unit is a TODO entry, not a file
- **Flat hierarchy**: All TODOs are presented at the same level for easy browsing
- **Full context**: Each entry includes all sub-items, properties, and content
- **Direct editing**: Edit entries directly and save back to their source files
- **File-agnostic workflow**: Work with TODOs without worrying about file organization

This approach can be extended to:
- GUI applications (using egui, iced, or Tauri)
- Web applications (using Dioxus or Yew)
- Mobile apps for org-mode task management

## Future Enhancements

- [x] ~~Edit mode for TODO entries~~ ✅ **Implemented!**
- [ ] TODO state toggling (mark as DONE, etc.)
- [ ] Search/filter functionality
- [ ] Agenda view (by date)
- [ ] Tag filtering
- [ ] Priority sorting
- [ ] Create new TODO entries
- [ ] Archive DONE entries
- [ ] Export/sync capabilities
- [ ] Calendar integration
- [ ] Undo/redo for edits
- [ ] Syntax highlighting in editor

## License

MIT
