# OrgStand

A powerful terminal-based TODO and agenda manager for org-mode files, written in Rust. OrgStand focuses on **TODO entries and notes** as the primary unit, making it easy to browse and manage tasks across multiple org files.

## Philosophy

OrgStand presents all TODO entries and notes from your org files in intelligent views. When you focus on an entry, it opens in a dedicated view where you can see its full content, edit it, manage dates, and organize tags.

This approach is inspired by tools like Logseq and Roam Research, where entries are first-class citizens rather than files.

## Features

### 📋 **Dual View System**
- **All TODOs**: Browse all TODO entries across files (filters out DONE items and Notes)
- **Week Agenda**: View all items scheduled for the current week (Monday-Sunday)
  - Shows items from all date types (SCHEDULED, DEADLINE, plain dates)
  - Displays time distance indicators ([Today], [+2d], [+1W], etc.)

### ✅ **TODO Management**
- Browse TODO entries with keyword highlighting (TODO, DONE, NEXT, WAITING, etc.)
- Toggle TODO states with a single key (`t`)
- View complete entries with all content, sub-tasks, and metadata
- Built-in full-text editor with save-to-file support
- Create new notes on the fly
- Delete entries with confirmation
- Support for Notes (entries without TODO keywords)

### 📅 **Advanced Date Management**
- **Three date types**:
  - SCHEDULED dates (`s` key)
  - DEADLINE dates (`d` key)
  - Plain dates (`p` key) - simple date markers without keywords
- Visual calendar picker with date and time selection
- Arrow key navigation in calendar
- Month navigation with `<`/`>` or PageUp/PageDown
- Week Agenda automatically shows nearest date from all types
- Intelligent date sorting (earliest first)

### 🏷️ **Smart Tag Management**
- ListView-based tag editor (no manual formatting needed!)
- Navigate tags with `↑`/`↓` or `k`/`j`
- **Click tags to edit** (mouse support!)
- Press `a` to add new tags
- Press `Enter` to edit selected tag
- Press `x` or `Delete` to remove tags
- Empty tag list shows helpful hints
- Tags are automatically formatted in org-mode style

### ⚡ **Quick Actions**
- **Quick Capture** (`c` key): Instantly create TODO scheduled for today
- **Help Screen** (`?` or `h` key): View all keybindings and features
- Tab switching between All TODOs and Week Agenda views
- Direct file editing with changes saved to original org files

### 🎨 **Rich Display**
- Color-coded TODO states (TODO: Red, DONE: Green, Others: Yellow)
- Multi-level heading colors
- Code blocks with borders
- Links with underlines
- Timestamps and properties
- File name display for each entry
- Empty state hints and guides

### 🔍 **Smart File Scanning**
- Recursively scans all `.org` files (up to 5 levels deep)
- Automatically skips hidden directories (`.git`, `.svn`, etc.)
- Skips large directories (`node_modules`, `target`, etc.)
- Safe to use in large directory trees
- Works with Beorg, Orgzly, and other mobile org apps

## Installation

```bash
cargo build --release
```

The binary will be available at `./target/release/orgstand`.

## Usage

### Browse TODO entries in current directory
```bash
./target/release/orgstand
# or during development:
cargo run
```

### Browse TODO entries in a specific directory
```bash
./target/release/orgstand /path/to/org/directory

# Example with Beorg files:
./target/release/orgstand ~/Library/Mobile\ Documents/iCloud~com~appsonthemove~beorg/Documents/org/
```

## Keyboard Controls

### 📖 Browser Mode (All TODOs / Week Agenda)
| Key | Action |
|-----|--------|
| `q` | Quit the application |
| `?` or `h` | Show help screen |
| `Tab` | Switch between All TODOs and Week Agenda |
| `↑`/`k` or `↓`/`j` | Navigate up/down in the list |
| `Enter` | Open selected TODO in viewer |
| `t` | Toggle TODO state (TODO ↔ DONE) |
| `s` | Set/edit SCHEDULED date |
| `d` | Set/edit DEADLINE date |
| `p` | Set/edit plain date |
| `e` | Edit TODO content in editor |
| `g` | Manage tags |
| `c` | Quick capture (create TODO for today) |
| `n` | Create new note |
| `x` or `Delete` | Delete TODO |

### 👁️ Viewer Mode (Entry View)
| Key | Action |
|-----|--------|
| `q` or `Esc` | Return to browser |
| `↑`/`k` or `↓`/`j` | Scroll up/down |
| `t` | Toggle TODO state |
| `s` / `d` / `p` | Set dates (same as browser) |
| `e` | Edit content |

### 📅 Date Input Mode (Calendar)
| Key | Action |
|-----|--------|
| `Arrows` | Navigate calendar (when editing date) |
| `<` / `>` or `PageUp`/`PageDown` | Change month |
| `Tab` | Switch between date and time editing |
| `↑`/`↓` | Adjust hours (when editing time) |
| `←`/`→` | Adjust minutes (when editing time) |
| `Enter` | Confirm and save |
| `Esc` | Cancel |

### ✏️ Editor Mode
| Key | Action |
|-----|--------|
| `Esc` or `Ctrl+S` | Save and exit |
| Normal keys | Edit text |

### 🏷️ Tag Management
**List Mode:**
| Key | Action |
|-----|--------|
| `↑`/`k` or `↓`/`j` | Navigate tags |
| `Enter` | Edit selected tag |
| `a` or `n` | Add new tag |
| `x` or `Delete` | Remove selected tag |
| `Esc` | Save and exit |
| **Mouse click** | Click tag to edit |

**Edit Mode:**
| Key | Action |
|-----|--------|
| `Enter` | Save tag |
| `Esc` | Cancel editing |
| Type | Edit tag name |

### ❓ Help Screen
| Key | Action |
|-----|--------|
| `q`, `Esc`, or `?` | Close help screen |
| `↑`/`k` or `↓`/`j` | Scroll help text |

## Date Display Format

In **All TODOs** view, each entry shows date distance to help prioritize:

| Format | Meaning |
|--------|---------|
| `[Today]` | Item is due today |
| `[Tmrw]` | Item is due tomorrow |
| `[Yday]` | Item was due yesterday |
| `[+Xd]` | Item is due in X days |
| `[+XW]` | Item is due in X weeks |
| `[+XM]` | Item is due in X months |
| `[-Xd]` | Item is overdue by X days |
| `[-XW]` | Item is overdue by X weeks |
| `[-XM]` | Item is overdue by X months |

**Note:** Date distance is shown in Week Agenda view, not in All TODOs view.

## Week Agenda View

The Week Agenda view shows all items scheduled for the current week (Monday-Sunday):
- Includes items with SCHEDULED, DEADLINE, or plain dates
- Automatically calculates week boundaries using calendar API
- Shows time distance for each item to indicate urgency
- Sorted by date (earliest first)

## Color Scheme

- **TODO Keywords**: Red (bold)
- **DONE Keywords**: Green (bold)
- **Other Keywords** (NEXT, WAITING, etc.): Yellow (bold)
- **Level 1 Headings**: Light Blue
- **Level 2 Headings**: Light Green
- **Level 3 Headings**: Light Yellow
- **Level 4 Headings**: Light Magenta
- **Links**: Blue (underlined)
- **Timestamps**: Magenta
- **Code Blocks**: Cyan
- **Properties/Drawers**: Dark Gray

## Examples

### Basic Usage
```bash
# Browse current directory
./target/release/orgstand .

# Browse Beorg files
./target/release/orgstand ~/Library/Mobile\ Documents/iCloud~com~appsonthemove~beorg/Documents/org/
```

### Common Workflows

**Daily Review:**
1. Press `Tab` to switch to Week Agenda
2. See all items for the current week with time indicators
3. Press `Enter` on an item to view details
4. Press `t` to mark items as DONE

**Adding a Task:**
1. Press `c` for quick capture
2. Type the task title
3. Press `Enter` - task is created and scheduled for today

**Managing Tags:**
1. Select a TODO and press `g`
2. Click on a tag to edit it, or press `a` to add new tags
3. Press `Esc` to save

**Setting Dates:**
1. Select a TODO and press `s` (SCHEDULED), `d` (DEADLINE), or `p` (plain date)
2. Navigate the calendar with arrow keys
3. Press `Tab` to edit time
4. Press `Enter` to save

## Architecture

OrgStand demonstrates a modern approach to org-mode:
1. **Entry-centric, not file-centric**: TODOs and notes are the primary units
2. **Intelligent views**: All TODOs and Week Agenda for different perspectives
3. **Full context**: Each entry includes all sub-items and metadata
4. **Direct editing**: Changes save back to source files automatically
5. **Cross-platform**: Works with any org-mode files (Emacs, Beorg, Orgzly, etc.)

Built with:
- `ratatui` for the TUI interface
- `orgize` for org-mode parsing
- `chrono` for date/time handling
- `tui-textarea` for text editing

## Future Enhancements

### Planned Features
- [ ] Delete date functionality (remove SCHEDULED/DEADLINE/plain dates)
- [ ] Priority support ([#A], [#B], [#C])
- [ ] Search and filter functionality
- [ ] Statistics display (TODO count, DONE count, etc.)
- [ ] Archive DONE entries
- [ ] More TODO state cycles (TODO → IN-PROGRESS → DONE)
- [ ] Repeating tasks (.+1d, +1w, etc.)
- [ ] Time tracking (Clock in/out)
- [ ] Sub-task support (checkbox lists)
- [ ] Configuration file (custom keywords, colors, etc.)

### Completed Features
- [x] Edit mode for TODO entries
- [x] TODO state toggling
- [x] Date management (SCHEDULED, DEADLINE, plain dates)
- [x] Calendar picker with time selection
- [x] Tag management
- [x] Week Agenda view
- [x] Help screen
- [x] Quick capture
- [x] Note creation
- [x] Entry deletion
- [x] Mouse support for tag editing

## License

MIT

## Contributing

Contributions welcome! Feel free to open issues or submit pull requests.

---

**Made with ❤️ for org-mode users who want a modern, efficient TODO management experience.**
