# rspin on niri - Setup Guide

## Quick Start

1. **Add window rule to niri config** (`~/.config/niri/config.kdl`):

```kdl
window-rule {
    match app-id="^rspin$"
    open-floating true
}
```

2. **Reload niri config**:
```bash
niri msg reload-config
```

3. **Run rspin** (GPU mode is default):
```bash
./target/release/rspin ~/path/to/image.jpg
```

The window will now appear as a floating overlay!

## Advanced Configuration

### Custom Position and Opacity

```kdl
window-rule {
    match app-id="^rspin$"
    open-floating true
    default-floating-position x=50 y=50 relative-to="top-left"
    opacity 0.9
    shadow {
        on
        softness 40
        spread 5
        color "#00000064"
    }
}
```

### Multiple Viewers with Different Behaviors

You can use custom app-ids for different use cases:

**For quick screenshot annotations:**
```bash
rspin --app-id rspin-screenshot screenshot.png
```

Config:
```kdl
window-rule {
    match app-id="^rspin-screenshot$"
    open-floating true
    default-floating-position x=0 y=0 relative-to="top-right"
    opacity 0.85
}
```

**For reference images while coding:**
```bash
rspin --app-id rspin-ref reference.png
```

Config:
```kdl
window-rule {
    match app-id="^rspin-ref$"
    open-floating true
    default-floating-position x=0 y=0 relative-to="bottom-right"
    default-column-width { fixed 600; }
}
```

## Comparison: GPU vs CPU Mode

| Feature | GPU Mode (default) | CPU Mode (`--no-gpu`) |
|---------|-------------------|----------------------|
| Performance | Better for large images | Good with optimizations |
| Window Type | Regular floating window | Layer-shell overlay |
| niri Config Required | Yes | No |
| Always On Top | Per-workspace only | **Global across all workspaces** |
| Visible when switching workspace | ❌ No (stays in original workspace) | ✅ Yes (always visible) |
| Works in other WMs | Yes | Only with wlr-layer-shell |

**For reference images you want to see everywhere: Use CPU mode (`--no-gpu`)**

## Keybindings Example

Add to niri config for quick access:

```kdl
binds {
    // Bind to open image picker with rspin
    Mod+Shift+V { spawn "bash" "-c" "rspin $(zenity --file-selection --file-filter='Images | *.png *.jpg *.jpeg *.gif *.webp')"; }
    
    // Screenshot and open with rspin
    Print { spawn "bash" "-c" "grim -g \"$(slurp)\" - | rspin"; }
}
```

## Troubleshooting

### Window is not floating
- Check if you reloaded niri config: `niri msg reload-config`
- Verify app-id matches: run `niri msg pick-window` and click the rspin window
- Check for typos in regex: it should be `"^rspin$"` with anchors

### GPU mode shows "Broken pipe" on very large windows
- This is fixed! The new version auto-limits texture size to 8192x8192
- If you still see issues, try CPU mode: `rspin --no-gpu image.png`

### Want it to work without config
- Use CPU mode: `rspin --no-gpu image.png`
- This uses layer-shell which is always floating
